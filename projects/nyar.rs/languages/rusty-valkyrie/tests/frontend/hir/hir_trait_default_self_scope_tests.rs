//! trait 默认体里的 `self` 作用域：只解析到本 trait 的成员槽。
//!
//! 声明期默认体的 `self` 没有实现类型，只有「本 trait 的身份」。成员查找必须先
//! 冻结在这个作用域里，不能落回全局同名候选池（这正是 `Comparable` 默认体
//! `self.compare` 报 9 candidate(s) share the name 的来源）。

use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::types::hir::HirStatementKind,
    types::hir::{HirExpr, HirExprKind, HirFunction, HirModule, HirResolvedCall, ValkyrieType},
};

use crate::primitives::SourceFixtureCompile;

fn compile(source: &str) -> HirModule {
    ValkyrieCompiler::default().compile_fixture_source(source).expect("trait 默认体必须完成 HIR")
}

fn trait_default<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module
        .traits
        .iter()
        .flat_map(|item| item.default_methods.iter())
        .find(|method| method.name.as_str() == name)
        .unwrap_or_else(|| panic!("trait 默认方法 `{name}` 必须存在"))
}

fn first_expr(method: &HirFunction) -> &HirExpr {
    method
        .body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            HirStatementKind::Let { initializer: Some(initializer), .. } => Some(initializer.as_ref()),
            HirStatementKind::Expr(expression) => Some(expression.as_ref()),
            _ => None,
        })
        .or(method.body.expr.as_deref())
        .expect("默认体必须包含调用表达式")
}

fn resolved_call(expr: &HirExpr) -> Option<&HirResolvedCall> {
    match &expr.kind {
        HirExprKind::Return(Some(inner)) => resolved_call(inner),
        HirExprKind::Call { resolved: Some(resolved), .. } => Some(resolved),
        _ => None,
    }
}

#[test]
fn default_body_self_resolves_to_own_trait_slot() {
    let module = compile(
        r#"
trait Meter {
    micro value(self): i64
    micro twice(self): i64 {
        return self.value()
    }
}
"#,
    );
    let twice = trait_default(&module, "twice");
    let resolved = resolved_call(first_expr(twice)).expect("`self.value()` 必须形成已解析调用合同");

    assert_eq!(resolved.symbol.to_string(), "Meter.value", "接收者 `self` 只能命中本 trait 的方法槽");
    assert!(resolved.instance.is_none(), "trait 槽在声明期没有实例身份");
    let slot = module
        .traits
        .iter()
        .flat_map(|item| item.methods.iter())
        .find(|method| method.name.as_str() == "value")
        .expect("trait 必须声明抽象成员 `value`");
    assert_eq!(resolved.declaration, slot.declaration, "冻结的声明必须是本 trait 的槽，而不是同名候选");

    let dispatch = resolved.trait_dispatch.as_ref().expect("trait 槽必须冻结成 witness");
    assert_eq!(dispatch.self_type, ValkyrieType::r#SelfType, "声明期 `self` 仍是 `Self` 占位");
}

#[test]
fn comparable_default_operators_reach_compare() {
    let module = compile(
        r#"
trait Comparable {
    micro compare(self, other: Self): i32

    infix `<`(self, other: Self): i32 {
        return self.compare(other)
    }

    infix `<=`(self, other: Self): i32 {
        return self.compare(other)
    }

    infix `>`(self, other: Self): i32 {
        return self.compare(other)
    }

    infix `>=`(self, other: Self): i32 {
        return self.compare(other)
    }
}

imply i32: Comparable {
    micro compare(self, other: i32): i32 {
        return other
    }
}
"#,
    );
    let comparable = module.traits.iter().find(|item| item.name.as_str() == "Comparable").expect("`Comparable` 必须出现在 HIR");
    assert_eq!(comparable.default_methods.len(), 4, "四个派生算子必须都是默认体");
    for method in &comparable.default_methods {
        let resolved = resolved_call(first_expr(method)).unwrap_or_else(|| panic!("`{}` 默认体的 `self.compare` 必须解析", method.name));
        assert_eq!(
            resolved.symbol.to_string(),
            "Comparable.compare",
            "`{}` 必须派发到本 trait 的 `compare`，got {}",
            method.name,
            resolved.symbol
        );
    }
}

#[test]
fn default_body_self_unknown_member_is_still_unresolved() {
    let failure = ValkyrieCompiler::default()
        .compile_source(&crate::primitives::with_primitives(
            r#"
trait Meter {
    micro value(self): i64
    micro twice(self): i64 {
        return self.missing()
    }
}
"#,
        ))
        .expect_err("本 trait 不存在的方法必须解析失败");
    let message = format!("{failure}");
    assert!(message.contains("unresolved"), "必须是 unresolved 结构化诊断，实际: {message}");
    assert!(message.contains("missing"), "诊断必须带上失败成员名，实际: {message}");
}
