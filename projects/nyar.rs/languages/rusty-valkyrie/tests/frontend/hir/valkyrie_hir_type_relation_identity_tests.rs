use rusty_valkyrie::{
    hir::type_relation::{ParameterMatchResult, TypeRelationContext},
    types::hir::{HirModule, HirTextEncoding, ValkyrieType},
    *,
};

use crate::primitives::SourceFixtureCompile;

#[test]
fn unresolved_primitive_and_array_spellings_do_not_create_type_equivalence() {
    let context = TypeRelationContext::from_module(&HirModule::default());
    for (spelling, actual) in [
        ("f64", ValkyrieType::Float64),
        ("std::primitive::f64", ValkyrieType::Float64),
        ("Utf8Text", ValkyrieType::Utf8),
        ("Array", ValkyrieType::Array(Box::new(ValkyrieType::Float64))),
    ] {
        let unresolved = ValkyrieType::Named(Identifier::new(spelling));
        assert!(matches!(context.match_parameter(&actual, &unresolved), ParameterMatchResult::NoMatch { .. }));
        assert!(matches!(context.match_parameter(&unresolved, &actual), ParameterMatchResult::NoMatch { .. }));
        assert_eq!(context.match_parameter(&actual, &actual), ParameterMatchResult::NominalExact);
    }
}

#[test]
fn unresolved_name_cannot_match_a_bound_nominal_declaration() {
    let hir = rusty_valkyrie::ValkyrieCompiler::default()
        .compile_fixture_source("structure Record { value: i32 }")
        .expect("结构声明解析");
    let context = TypeRelationContext::from_module(&hir);
    // 共享 primitive 前导也在 `structs` 里，必须按声明名取被测结构而不是取首项。
    let item = hir.structs.iter().find(|item| item.name.as_str() == "Record").expect("`Record` 声明");
    let nominal = ValkyrieType::Nominal {
        declaration: item.constructor_declaration.expect("冻结声明身份"),
        arguments: vec![],
        primitive_identity: item.primitive_identity.clone(),
    };
    let unresolved = ValkyrieType::Named(item.name.clone());
    assert!(matches!(context.match_parameter(&nominal, &unresolved), ParameterMatchResult::NoMatch { .. }));
    assert!(matches!(context.match_parameter(&unresolved, &nominal), ParameterMatchResult::NoMatch { .. }));
    assert_eq!(context.match_parameter(&nominal, &nominal), ParameterMatchResult::NominalExact);
}

/// 用户声明的同名 `class Utf8Text` 没有 `[primitive]` 身份，不获得文本语义。
#[test]
fn user_class_named_like_text_carrier_has_no_text_identity() {
    let hir = rusty_valkyrie::ValkyrieCompiler::default().compile_fixture_source("class Utf8Text { }").expect("用户声明解析");
    let context = TypeRelationContext::from_module(&hir);
    let item = hir.structs.iter().find(|item| item.name.as_str() == "Utf8Text").expect("`Utf8Text` 声明");
    let nominal = ValkyrieType::Nominal {
        declaration: item.constructor_declaration.expect("冻结声明身份"),
        arguments: vec![],
        primitive_identity: item.primitive_identity.clone(),
    };
    assert!(!context.is_text_formal(&nominal), "同名用户声明不得被当成文本形参");
    assert_eq!(context.text_encoding_of(&nominal), None, "同名用户声明不得产生文本编码");
}

/// `[primitive("std::text::…")]` 是文本身份的唯一来源：编码由声明身份决定。
#[test]
fn primitive_text_declaration_carries_encoding_identity() {
    for (identity, carrier, encoding) in [
        ("std::text::utf8", ValkyrieType::Utf8, HirTextEncoding::Utf8),
        ("std::text::utf16", ValkyrieType::Utf16, HirTextEncoding::Utf16),
        ("std::text::text", ValkyrieType::Utf8, HirTextEncoding::Utf8),
    ] {
        let hir = rusty_valkyrie::ValkyrieCompiler::default()
            .compile_fixture_source(&format!("[primitive(\"{identity}\")] class Utf8Text {{ }}"))
            .expect("文本声明解析");
        let context = TypeRelationContext::from_module(&hir);
        let item = hir.structs.iter().find(|item| item.name.as_str() == "Utf8Text").expect("`Utf8Text` 声明");
        let nominal = ValkyrieType::Nominal {
            declaration: item.constructor_declaration.expect("冻结声明身份"),
            arguments: vec![],
            primitive_identity: item.primitive_identity.clone(),
        };
        assert!(context.is_text_formal(&nominal), "`{identity}` 必须被识别为文本形参");
        assert_eq!(context.text_encoding_of(&nominal), Some(encoding), "`{identity}` 编码");
        // 文本身份是声明级事实，不进入类型身份：文本声明的 `Nominal` 不得携带 primitive 身份。
        assert_eq!(nominal.primitive_identity(), None, "文本身份不得进入 `Nominal`");
        assert_eq!(item.text_carrier_identity.as_deref(), Some(identity), "声明级文本身份必须保留");
        assert_eq!(context.text_encoding_of(&carrier), Some(encoding));
    }
}

/// 同命名空间的 `type utf8 = <文本声明>` 别名解析后仍保留文本身份。
#[test]
fn text_identity_survives_same_module_alias() {
    let hir = rusty_valkyrie::ValkyrieCompiler::default()
        .compile_fixture_source(
            r#"
namespace std.text;
[primitive("std::text::utf8")]
class Utf8Text { }

type utf8 = Utf8Text;

micro take(value: utf8) { }
"#,
        )
        .expect("别名声明解析");
    let context = TypeRelationContext::from_module(&hir);
    let function = hir.functions.iter().find(|function| function.name.as_str() == "take").expect("`take` 函数");
    let parameter = &function.params[0].ty;
    assert!(context.is_text_formal(parameter), "别名解析后仍是文本形参: {parameter:?}");
    assert_eq!(context.text_encoding_of(parameter), Some(HirTextEncoding::Utf8));
}
