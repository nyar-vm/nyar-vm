//! CLR 容器载体合同：数组、引用布局与可空形状的类型描述符。
//!
//! 与 JVM 用例镜像：直接构造冻结的 `AggregateLayoutPlan` 与 `NyarType`，
//! 只验证载体分类，不经过前端或完整 lowering。

use nyar::Identifier;
use nyar_emitter::{nyar_backend_clr::MsilType, testing::msil_type};
use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldLayout, NyarType, StorageKind};

/// `Option<char>` 形态的引用存储布局：身份 `0`，非空字段。
fn reference_layout_plan() -> AggregateLayoutPlan {
    AggregateLayoutPlan {
        layouts: vec![AggregateLayout {
            id: 0,
            name: "Option".to_string(),
            namespace: "core".to_string(),
            key: None,
            storage: StorageKind::Reference,
            size: 8,
            align: 4,
            fields: vec![
                FieldLayout { name: "discriminant".to_string(), ty: NyarType::Integer32 { signed: false }, offset: 0, size: 4, align: 4 },
                FieldLayout { name: "payload".to_string(), ty: NyarType::Character, offset: 4, size: 4, align: 4 },
            ],
        }],
        ..AggregateLayoutPlan::default()
    }
}

fn array_of(element: NyarType) -> NyarType {
    NyarType::Array(Box::new(element))
}

#[test]
fn array_of_integer64_uses_int64_carrier() {
    let ty = msil_type(&reference_layout_plan(), &array_of(NyarType::Integer64 { signed: true })).expect("i64 array");
    assert_eq!(ty, MsilType::sz_array(MsilType::Int64 { signed: true }));
}

#[test]
fn array_of_integer8_uses_int8_carrier() {
    let ty = msil_type(&reference_layout_plan(), &array_of(NyarType::Integer8 { signed: false })).expect("u8 array");
    assert_eq!(ty, MsilType::sz_array(MsilType::Int8 { signed: false }));
}

#[test]
fn array_of_character_uses_char_carrier() {
    let ty = msil_type(&reference_layout_plan(), &array_of(NyarType::Character)).expect("char array");
    assert_eq!(ty, MsilType::sz_array(MsilType::Char));
}

#[test]
fn nominal_reference_layout_uses_generated_type_name() {
    let plan = reference_layout_plan();
    let nominal = NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") };
    let ty = msil_type(&plan, &nominal).expect("reference nominal");
    let MsilType::Named(name) = ty
    else {
        panic!("reference nominal must use a named carrier, got {ty:?}");
    };
    assert!(name.contains("nyar::layout::"), "{name}");
    assert!(name.ends_with("$0"), "type name must be driven by layout identity: {name}");
}

#[test]
fn integer128_is_rejected_structurally() {
    let error = msil_type(&reference_layout_plan(), &NyarType::Integer128 { signed: true }).expect_err("i128 has no CLR carrier");
    assert!(error.contains("i128"), "{error}");
}

#[test]
fn bare_tuple_is_rejected_structurally() {
    let error = msil_type(&reference_layout_plan(), &NyarType::Tuple(vec![NyarType::Integer32 { signed: true }]))
        .expect_err("bare tuple has no layout identity");
    assert!(error.contains("裸 tuple"), "{error}");
}

#[test]
fn nullable_scalar_is_rejected_structurally() {
    let error = msil_type(&reference_layout_plan(), &NyarType::Nullable(Box::new(NyarType::Integer32 { signed: true })))
        .expect_err("nullable scalar needs a sum representation");
    assert!(error.contains("sum 表示"), "{error}");
}
