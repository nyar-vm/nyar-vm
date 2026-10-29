//! 泛型接收者上的 trait 方法调用必须在 HIR 冻结分派义务（evidence），且不做单态化。

use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::types::hir::{HirExprKind, ValkyrieType},
};

const SOURCE: &str = r#"
namespace test.dispatch;

trait Echo {
    micro echo(self) -> bool
}

micro use_echo<T>(value: T) -> bool where T: Echo {
    return value.echo()
}
"#;

#[test]
fn generic_receiver_trait_method_freezes_dispatch_evidence() {
    let hir = ValkyrieCompiler::default().compile_source(SOURCE).expect("带 where 约束的泛型调用必须可解析");
    let echo_trait = hir.traits.iter().find(|item| item.name.as_str() == "Echo").expect("Echo 声明");
    let trait_declaration = echo_trait.declaration.expect("trait 声明身份");
    let method = echo_trait.methods.iter().find(|method| method.name.as_str() == "echo").expect("echo 方法声明");
    let method_declaration = method.declaration.expect("方法声明身份");

    let function = hir.functions.iter().find(|function| function.name.as_str() == "use_echo").expect("use_echo HIR");
    let returned = match function.body.expr.as_deref().map(|expression| &expression.kind) {
        Some(HirExprKind::Return(Some(value))) => value,
        other => panic!("expected `return ...` block expression, got {other:?}"),
    };
    let resolved = match &returned.kind {
        HirExprKind::Call { resolved: Some(resolved), .. } => resolved,
        other => panic!("trait 方法调用必须有已解析合同，got {other:?}"),
    };

    assert!(resolved.instance.is_none(), "泛型 trait 方法没有可执行实例身份，不得伪造");
    let dispatch = resolved.trait_dispatch.as_ref().expect("必须冻结 trait 分派义务");
    assert_eq!(dispatch.trait_declaration, trait_declaration);
    assert_eq!(dispatch.method_declaration, method_declaration);
    match &dispatch.self_type {
        ValkyrieType::Generic(generic) => {
            assert_eq!(generic.name.as_str(), "T", "self 类型必须保持开放泛型，不得单态化");
            assert!(generic.binder.is_some(), "开放泛型必须保留 binder");
        }
        other => panic!("expected open Generic(T), got {other:?}"),
    }
}
