//! 块 2：`[constructor]` 变体的构造 callable 合成合同。
//!
//! 只断言声明身份与签名：`owner` / `symbol` / `declaration` / `sum_owner` / `sum_variant`、
//! 参数顺序、返回类型与泛型 binder 都来自变体声明本身，不按变体短名建表。

use std::collections::BTreeSet;

use rusty_valkyrie::{
    ValkyrieCompiler,
    hir::overload::{OverloadCandidate, OverloadDomain, collect_module_candidates},
    types::hir::{HirEnum, HirModule, ValkyrieType},
};

fn compile(source: &str) -> HirModule {
    ValkyrieCompiler::default().compile_source(source).unwrap_or_else(|error| panic!("源码必须完成 HIR lowering: {error}"))
}

fn enum_named<'a>(module: &'a HirModule, name: &str) -> &'a HirEnum {
    module.enums.iter().find(|item| item.name.as_str() == name).unwrap_or_else(|| panic!("缺少 {name} 声明"))
}

fn variant_auto_constructor<'a>(candidates: &'a [OverloadCandidate], owner: &str, symbol: &str) -> &'a OverloadCandidate {
    candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Constructor)
        .filter(|candidate| candidate.owner.as_ref().is_some_and(|owner_name| owner_name.as_str() == owner))
        .find(|candidate| matches!(candidate.symbol.parts(), [part] if part.as_str() == symbol))
        .unwrap_or_else(|| panic!("缺少 {owner}::{symbol} 构造候选"))
}

fn assert_nominal_return(return_type: &ValkyrieType, declaration: nyar_types::ItemId, arguments_len: usize) {
    match return_type {
        ValkyrieType::Nominal { declaration: actual, arguments, .. } => {
            assert_eq!(*actual, declaration, "构造返回类型必须指向 unite 声明身份");
            assert_eq!(arguments.len(), arguments_len, "构造返回类型的泛型实参数量不符: {return_type:?}");
        }
        other => panic!("构造返回类型必须是 Nominal，got {other:?}"),
    }
}

#[test]
fn single_field_generic_variant_synthesizes_declaration_bound_constructor() {
    let module = compile(
        r#"
unite Option<T> {
    [constructor]
    Some {
        value: T,
    },
    None,
}
"#,
    );
    let candidates = collect_module_candidates(&module);
    let option = enum_named(&module, "Option");
    let some = &option.variants[0];
    let candidate = variant_auto_constructor(&candidates, "Option", "Some");

    assert!(candidate.is_auto_constructor, "Some 必须携带 [constructor] 标记");
    assert_eq!(candidate.declaration, some.declaration, "构造候选必须绑定变体声明身份");
    assert_eq!(candidate.sum_owner, option.declaration, "构造候选必须绑定 unite 声明身份");
    assert_eq!(candidate.sum_variant, some.declaration, "构造候选必须绑定变体声明身份");
    assert_eq!(candidate.signature.params, some.fields.iter().map(|field| field.ty.clone()).collect::<Vec<_>>());
    assert_eq!(candidate.signature.params.len(), 1);
    assert_nominal_return(&candidate.signature.return_type, option.declaration.expect("Option declaration"), 1);

    let expected_binders = option.generics.iter().filter_map(|generic| generic.binder).collect::<BTreeSet<_>>();
    assert_eq!(candidate.generic_binders, expected_binders, "构造候选的泛型 binder 必须来自 unite 声明");
}

#[test]
fn multi_field_variant_keeps_declaration_order_in_positional_parameters() {
    let module = compile(
        r#"
unite Pair {
    [constructor]
    Both {
        left: bool,
        right: utf8,
    },
    Neither,
}
"#,
    );
    let candidates = collect_module_candidates(&module);
    let pair = enum_named(&module, "Pair");
    let both = &pair.variants[0];
    let candidate = variant_auto_constructor(&candidates, "Pair", "Both");

    assert!(candidate.is_auto_constructor);
    assert_eq!(candidate.signature.params, vec![ValkyrieType::Boolean, ValkyrieType::Utf8], "参数必须按字段声明顺序");
    assert_eq!(candidate.signature.params, both.fields.iter().map(|field| field.ty.clone()).collect::<Vec<_>>());
    assert_nominal_return(&candidate.signature.return_type, pair.declaration.expect("Pair declaration"), 0);
}

/// 01 §4.11：`[constructor]` 标注无字段体裸变体是非法声明，必须 fail-closed；
/// 零载荷变体的构造 callable 合成合同已退役，不另发明裸变体构造/调用语法。
#[test]
fn zero_payload_variant_constructor_synthesis_is_retired() {
    let error = ValkyrieCompiler::default()
        .compile_source(
            r#"
unite Flag {
    [constructor]
    Off,
    On,
}
"#,
        )
        .expect_err("裸变体 [constructor] 必须 fail-closed");
    assert!(
        matches!(error, rusty_valkyrie::HirError::AutoConstructorAttributeOnFieldlessUnityVariant { .. }),
        "期望 AutoConstructorAttributeOnFieldlessUnityVariant，got {error:?}"
    );
}

#[test]
fn variant_without_constructor_attribute_has_candidates_but_not_the_call_entry() {
    let module = compile(
        r#"
unite Plain<T> {
    Present {
        value: T,
    },
    Absent,
}
"#,
    );
    let candidates = collect_module_candidates(&module);
    let plain = enum_named(&module, "Plain");

    for variant in &plain.variants {
        let candidate = variant_auto_constructor(&candidates, "Plain", variant.name.as_str());
        assert!(!candidate.is_auto_constructor, "{} 未标注 [constructor]，括号调用入口必须关闭", variant.name);
        assert_eq!(candidate.signature.params, variant.fields.iter().map(|field| field.ty.clone()).collect::<Vec<_>>());
        assert_eq!(candidate.sum_variant, variant.declaration);
    }
}

#[test]
fn constructor_attribute_only_marks_the_annotated_variant() {
    let module = compile(
        r#"
unite Mixed {
    [constructor]
    Wrapped {
        value: bool,
    },
    Bare,
}
"#,
    );
    let candidates = collect_module_candidates(&module);
    let mixed = enum_named(&module, "Mixed");

    assert!(variant_auto_constructor(&candidates, "Mixed", "Wrapped").is_auto_constructor);
    assert!(!variant_auto_constructor(&candidates, "Mixed", "Bare").is_auto_constructor);
    assert_eq!(mixed.variants.len(), 2);
}
