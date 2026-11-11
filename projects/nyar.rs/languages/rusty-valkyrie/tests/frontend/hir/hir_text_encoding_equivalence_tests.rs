//! 文本编码的类型表示归一合同。
//!
//! 生成源码组（`GeneratedOnly`）里的 `utf8` 按合同 01 §4 回落内建载体
//! `ValkyrieType::Utf8`，而依赖侧 `type utf8 = Utf8Text` 把形参冻结成
//! `Nominal{ItemId}`。二者是同一值空间，必须参与同一重载候选；跨编码
//! （`Utf8` vs `Utf16Text`）与无关名义声明必须继续 fail-closed。

use rusty_valkyrie::{
    hir::type_relation::{ParameterMatchResult, TypeRelationContext},
    types::hir::ValkyrieType,
    *,
};

use crate::primitives::SourceFixtureCompile;

fn nominals() -> (TypeRelationContext, [(ValkyrieType, &'static str); 3]) {
    let hir = ValkyrieCompiler::default()
        .compile_fixture_source(
            "[primitive(\"std::text::utf8\")] structure Utf8Text { _repr: i32 }\n\
             [primitive(\"std::text::utf16\")] structure Utf16Text { _repr: i32 }\n\
             structure Widget { value: i32 }",
        )
        .expect("测试名义声明必须解析");
    let context = TypeRelationContext::from_module(&hir);
    let nominal = |name: &str| {
        let item = hir.structs.iter().find(|item| item.name.as_str() == name).unwrap_or_else(|| panic!("`{name}` 必须被收集"));
        ValkyrieType::Nominal {
            declaration: item.constructor_declaration.expect("冻结声明身份"),
            arguments: vec![],
            primitive_identity: item.primitive_identity.clone(),
        }
    };
    (
        context,
        [
            (nominal("Utf8Text"), "Utf8Text"),
            (nominal("Utf16Text"), "Utf16Text"),
            (nominal("Widget"), "Widget"),
        ],
    )
}

#[test]
fn builtin_utf8_matches_nominal_utf8_text() {
    let (context, nominals) = nominals();
    assert_eq!(context.match_parameter(&ValkyrieType::Utf8, &nominals[0].0), ParameterMatchResult::NominalExact);
}

#[test]
fn builtin_utf8_rejects_nominal_utf16_text() {
    let (context, nominals) = nominals();
    assert!(matches!(context.match_parameter(&ValkyrieType::Utf8, &nominals[1].0), ParameterMatchResult::NoMatch { .. }));
}

#[test]
fn builtin_utf16_rejects_nominal_utf8_text() {
    let (context, nominals) = nominals();
    assert!(matches!(context.match_parameter(&ValkyrieType::Utf16, &nominals[0].0), ParameterMatchResult::NoMatch { .. }));
}

#[test]
fn builtin_utf8_rejects_unrelated_nominal() {
    let (context, nominals) = nominals();
    assert!(matches!(context.match_parameter(&ValkyrieType::Utf8, &nominals[2].0), ParameterMatchResult::NoMatch { .. }));
}
