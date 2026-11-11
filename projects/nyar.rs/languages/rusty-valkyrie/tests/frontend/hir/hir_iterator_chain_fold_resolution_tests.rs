//! 迭代器链尾 `fold` 的方法位置解析：`fold` 是 `std.iterator` 的扩展自由函数
//! （首参 `self`），其类型参数 `U` 由 `initial` 与闭包签名共同约束。
//!
//! 回归：`fold(0, micro(accumulator: i64, item: i64) -> i64 { … })` 里的无约束整数字面量
//! `0` 不得先于闭包签名把 `U` 钉成默认宽度 `i32`，否则候选在统一阶段整体失效，退化成
//! 「`fold` 无候选」。字面量必须延后到其它实参解出 binder 之后再定型（01 §4.10 B）。

use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::types::hir::{HirExpr, HirExprKind, HirModule, HirStatementKind, ValkyrieType},
    hir::lowering::DEFAULT_COMPILE_ARCH,
};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::u64")]
structure u64 { }

type usize = u64;

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i64")]
structure i64 { }

imply i64 {
    infix `+`(self, other: Self): Self {
        return self
    }

    infix `*`(self, other: Self): Self {
        return self
    }

    infix `%`(self, other: Self): Self {
        return self
    }

    infix `>`(self, other: Self): bool {
        return false
    }

    infix `!=`(self, other: Self): bool {
        return false
    }
}
"#;

/// `std.iterator` 形态：trait 适配器、imply 实现与尾 `fold` 扩展函数。
const ITERATOR: &str = r#"
namespace std.iterator;

using std.primitive;

trait Iterator {
    type Item;

    micro skip(self, count: usize) -> SkipIterator<Item, Self> {
        return SkipIterator { _iter: self, _count: count }
    }

    micro take(self, count: usize) -> TakeIterator<Item, Self> {
        return TakeIterator { _iter: self, _count: count }
    }

    micro map<U>(self, f: micro(Item) -> U) -> MapIterator<Item, U, Self> {
        return MapIterator { _iter: self, _mapper: f }
    }

    micro filter(self, pred: micro(Item) -> bool) -> FilterIterator<Item, Self> {
        return FilterIterator { _iter: self, _predicate: pred }
    }
}

structure SkipIterator<T, I> {
    _iter: I
    _count: usize
}

imply SkipIterator<T, I>: Iterator {
    type Item = T;
}

structure TakeIterator<T, I> {
    _iter: I
    _count: usize
}

imply TakeIterator<T, I>: Iterator {
    type Item = T;
}

structure MapIterator<T, U, I> {
    _iter: I
    _mapper: micro(T) -> U
}

imply MapIterator<T, U, I>: Iterator {
    type Item = U;
}

structure FilterIterator<T, I> {
    _iter: I
    _predicate: micro(T) -> bool
}

imply FilterIterator<T, I>: Iterator {
    type Item = T;
}

structure Base<T> {
    _seed: T
}

imply Base<T>: Iterator {
    type Item = T;
}

micro fold<I, T, U>(self: I, initial: U, f: micro(U, T) -> U): U
    where I: Iterator<Item = T>
{
    return initial
}
"#;

const APP_HEAD: &str = r#"
namespace probe.chain;

using std.primitive;
using std.iterator;

micro is_odd(value: i64) -> bool {
    return value % 2 != 0
}

micro triple(value: i64) -> i64 {
    return value * 3
}

micro above_fifteen(value: i64) -> bool {
    return value > 15
}
"#;

fn resolve(consumer: &str) -> Vec<HirModule> {
    let groups = [
        CompilerSourceGroup::from_source_files(
            "std",
            "std",
            vec![
                rusty_valkyrie::CompilerSourceFile { path: "primitives.v".into(), source: PRIMITIVES.into() },
                rusty_valkyrie::CompilerSourceFile { path: "iterator.v".into(), source: ITERATOR.into() },
            ],
            Vec::new(),
        ),
        CompilerSourceGroup::single_source("app", "app", &format!("{APP_HEAD}\n{consumer}"))
            .as_entry()
            .with_direct_dependencies(vec!["std".into()]),
    ];
    ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .map_err(|error| format!("{error:?}"))
        .expect("链尾 `fold` 必须解析")
}

/// `run` 函数体尾 `return <chain>` 的最外层调用表达式。
fn tail_call<'a>(module: &'a HirModule) -> &'a HirExpr {
    let function = module.functions.iter().find(|function| function.name.as_str() == "run").expect("fixture 缺少 `run`");
    let expression = match (&function.body.expr, function.body.statements.last()) {
        (Some(expression), _) => expression,
        (None, Some(statement)) => match &statement.kind {
            HirStatementKind::Expr(expr) => expr,
            other => panic!("`run` 的最后一条语句必须是表达式，实际 {other:?}"),
        },
        (None, None) => panic!("`run` 缺少语句"),
    };
    match &expression.kind {
        HirExprKind::Return(Some(inner)) => inner,
        other => panic!("`run` 必须以 `return` 收尾，实际 {other:?}"),
    }
}

/// 尾调用必须绑定到 `std.iterator::fold` 的声明实例，而不是按短名撞到别的成员。
fn assert_fold_is_resolved_to_declaration(module: &HirModule) {
    let call = tail_call(module);
    let HirExprKind::Call { resolved: Some(resolved), .. } = &call.kind
    else {
        panic!("链尾必须是已解析的 `fold` 调用");
    };
    assert_eq!(resolved.symbol.parts().last().map(|part| part.as_str()), Some("fold"));
    assert!(resolved.declaration.is_some(), "`fold` 必须冻结声明身份");
    assert!(resolved.instance.is_some(), "`fold` 的泛型实参必须完成代入");
    assert_eq!(resolved.return_type, ValkyrieType::Integer64 { signed: true }, "`fold` 的 `U` 必须由闭包签名解成 `i64`");
    assert_eq!(resolved.parameter_types.len(), 3, "`fold` 的接收者 + 初始值 + 函数实参必须齐全");
}

fn app_module(modules: &[HirModule]) -> &HirModule {
    modules.iter().find(|module| module.name.to_string().contains("app")).expect("缺少 app 模块")
}

#[test]
fn fold_with_lambda_and_bare_literal_initial_resolves() {
    let modules = resolve(
        r#"
[export] micro run(base: Base<i64>) -> i64 {
    return base.filter(is_odd).map(triple).skip(2).take(4).filter(above_fifteen).fold(0, micro(accumulator: i64, item: i64) -> i64 { return accumulator + item })
}
"#,
    );
    assert_fold_is_resolved_to_declaration(app_module(&modules));
}

/// `0 as i64` 这类已定型初始值不得回归。
#[test]
fn fold_with_typed_literal_initial_resolves() {
    let modules = resolve(
        r#"
[export] micro run(base: Base<i64>) -> i64 {
    return base.filter(is_odd).map(triple).skip(2).take(4).filter(above_fifteen).fold(0 as i64, micro(accumulator: i64, item: i64) -> i64 { return accumulator + item })
}
"#,
    );
    assert_fold_is_resolved_to_declaration(app_module(&modules));
}

/// 真实语料形态：每个适配器都是语言正式的 `micro(…) -> T { … }` 闭包。
///
/// 该形态的类型嵌套深度会消耗较多栈（推断期递归与类型深度成正比），测试线程默认
/// 栈只有 2 MiB，故在专用栈线程里执行；这不是放宽任何解析规则。
#[test]
fn fold_after_lambda_adapter_chain_resolves() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let modules = resolve(
                r#"
[export] micro run(base: Base<i64>) -> i64 {
    return base.filter(micro(x: i64) -> bool { return x % 2 != 0 }).map(micro(x: i64) -> i64 { return x * 3 }).skip(2).take(4).filter(micro(x: i64) -> bool { return x > 15 }).fold(0, micro(accumulator: i64, item: i64) -> i64 { return accumulator + item })
}
"#,
            );
            assert_fold_is_resolved_to_declaration(app_module(&modules));
        })
        .expect("spawn")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}
