//! 依赖命名空间导入后的调用解析合同。
//!
//! 锁定两条已经成立的自由函数调用事实：
//! 1. `using` 引入的依赖命名空间中，顶层泛型自由函数能以单段短名进入候选集并完成泛型推断；
//! 2. 形参本身是函数类型时，实参按局部类型匹配。
//!
//! 未锁定（当前不成立）：具名函数作为函数类型实参（`f(x, named_fn)`）。该实参在
//! `infer_expr_type` 中没有任何类型来源，候选匹配因此恒失败，属于 HIR 缺少函数引用
//! 表达式这一层的欠账，不是命名空间候选集问题。

use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler, hir::lowering::DEFAULT_COMPILE_ARCH};

/// 依赖组：命名空间内声明一个泛型自由函数。
const LIBRARY: &str = r#"
namespace std.collection;

structure Crate<T> { }

micro arrange<T>(items: Crate<T>, compare: micro(T, T) -> i32): unit {
    return
}
"#;

/// consumer：`using` 引入命名空间，单段短名调用依赖组的泛型函数，
/// 并把函数类型形参原样转发（该实参有局部类型，因此不依赖函数引用表达式）。
const CONSUMER: &str = r#"
namespace app;

using std.collection;

[export] micro run(items: Crate<i64>, compare: micro(i64, i64) -> i32): unit {
    arrange(items, compare)
    return
}
"#;

#[test]
fn imported_namespace_single_segment_call_resolves_item_and_generic_binder() {
    let groups = [
        CompilerSourceGroup::single_source("std", "std", LIBRARY),
        CompilerSourceGroup::single_source("app", "app", CONSUMER).as_entry().with_direct_dependencies(vec!["std".into()]),
    ];
    ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).expect("`using` 引入的依赖函数必须能以单段短名解析");
}
