//! `[constructor]` 变体属性的 AST/HIR 解析保留与 fail-closed 测试。

use rusty_valkyrie::{HirError, ValkyrieCompiler};

const CONSTRUCTOR_UNITE: &str = r#"
unite Option<T> {
    [constructor]
    Some {
        value: T,
    },
    None,
}
"#;

#[test]
fn constructor_attribute_is_retained_on_the_variant_declaration() {
    let module = ValkyrieCompiler::default().compile_source(CONSTRUCTOR_UNITE).expect("带 [constructor] 的 unite 必须完成 HIR lowering");
    let option = module.enums.iter().find(|item| item.name.to_string() == "Option").expect("Option 声明");
    assert_eq!(option.variants.len(), 2);
    assert!(option.variants[0].is_auto_constructor);
    assert!(!option.variants[1].is_auto_constructor);
    assert_eq!(option.variants[0].name.to_string(), "Some");
    assert_eq!(option.variants[0].fields.len(), 1);
    assert_eq!(option.variants[0].fields[0].name.to_string(), "value");
}

#[test]
fn unite_without_constructor_attribute_keeps_the_same_shape() {
    let module = ValkyrieCompiler::default()
        .compile_source(
            r#"
unite Plain<T> {
    Present {
        value: T,
    },
    Absent,
}
"#,
        )
        .expect("无 [constructor] 的 unite 必须继续完成 HIR lowering");
    let plain = module.enums.iter().find(|item| item.name.to_string() == "Plain").expect("Plain 声明");
    assert_eq!(plain.variants.len(), 2);
    assert!(!plain.variants[0].is_auto_constructor);
    assert!(!plain.variants[1].is_auto_constructor);
    assert_eq!(plain.variants[0].fields.len(), 1);
}

#[test]
fn constructor_attribute_on_type_enum_variant_is_rejected() {
    let error = ValkyrieCompiler::default().compile_source("enums Choice { [constructor] First }").expect_err("非 unite 变体必须 fail-closed");
    match error {
        HirError::AutoConstructorAttributeOnNonUnityVariant { aggregate, variant, declared_kind, .. } => {
            assert_eq!(aggregate, "Choice");
            assert_eq!(variant, "First");
            assert_eq!(declared_kind, "enums");
        }
        other => panic!("期望 AutoConstructorAttributeOnNonUnityVariant，got {other:?}"),
    }
}

#[test]
fn duplicate_constructor_attribute_is_rejected() {
    let error = ValkyrieCompiler::default()
        .compile_source("unite Wrapped { [constructor] [constructor] Empty }")
        .expect_err("同一变体重复标注必须 fail-closed");
    match error {
        HirError::DuplicateAutoConstructorAttribute { aggregate, variant, .. } => {
            assert_eq!(aggregate, "Wrapped");
            assert_eq!(variant, "Empty");
        }
        other => panic!("期望 DuplicateAutoConstructorAttribute，got {other:?}"),
    }
}

#[test]
fn constructor_attribute_on_structure_is_rejected() {
    let error = ValkyrieCompiler::default()
        .compile_source("[constructor] structure Holder { }")
        .expect_err("结构体上的 [constructor] 必须 fail-closed");
    match error {
        HirError::AutoConstructorAttributeOnNonVariant { target_kind, name, .. } => {
            assert_eq!(target_kind, "structure");
            assert_eq!(name, "Holder");
        }
        other => panic!("期望 AutoConstructorAttributeOnNonVariant，got {other:?}"),
    }
}

#[test]
fn constructor_attribute_on_micro_is_rejected() {
    let error = ValkyrieCompiler::default()
        .compile_source("[constructor] micro entry() -> i32 { return 0 }")
        .expect_err("函数上的 [constructor] 必须 fail-closed");
    match error {
        HirError::AutoConstructorAttributeOnNonVariant { target_kind, name, .. } => {
            assert_eq!(target_kind, "micro");
            assert_eq!(name, "entry");
        }
        other => panic!("期望 AutoConstructorAttributeOnNonVariant，got {other:?}"),
    }
}
