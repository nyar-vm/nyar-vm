//! primitive 载体必须由声明携带的 `[primitive("…")]` 规范身份决定。
//!
//! 修复前 `compute_aggregate_layout_plan` 用 `numeric_builtin_cast_type(struct.name)`
//! 按**短名**登记 carrier：任何零字段 `structure i32 { }` 都会被当成内建 i32 载体，
//! 哪怕它的 `primitive_identity` 是另一条身份（`acme::i32`）或根本没有身份。
//! `layout_field_type` 也曾按拼写猜载体；公共 `nyar-types` 的拼写桥已删除，
//! 语言表面拼写分类只留在前端，数值载体一律由冻结身份结算。
//!
//! 这里用两个反例把“名字”和“身份”分开：
//! - 同名但不同身份（`acme::i32`）：字段必须落在该声明自己的名义布局，而不是 i32 载体；
//! - 标注漂移（名字 `i32`、身份 `std::primitive::u64`）：字段必须落在 u64 载体。

use nyar::NyarType;
use rusty_valkyrie::{
    Identifier, MirLowerer, ValkyrieCompiler,
    compiler::{
        mir::ssa::{AggregateLayout, MirModule},
        types::hir::{HirField, HirModule, HirStruct},
    },
    compute_aggregate_layout_plan,
    types::hir::types::{PrimitiveNumericShape, ValkyrieType, numeric_primitive_identity_shape},
};

fn lower(source: &str) -> MirModule {
    let hir = ValkyrieCompiler::default().compile_source(source).expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

fn layout<'a>(module: &'a MirModule, name: &str) -> &'a AggregateLayout {
    module.aggregate_layouts.layouts.iter().find(|layout| layout.name == name).unwrap_or_else(|| {
        panic!(
            "缺少 `{name}` 布局，已登记: {:?}",
            module.aggregate_layouts.layouts.iter().map(|layout| layout.name.clone()).collect::<Vec<_>>()
        )
    })
}

fn holder_field(module: &MirModule) -> &nyar_types::FieldLayout {
    &layout(module, "Holder").fields[0]
}

/// 反例（本次修复的观测点）：用户 struct 与内建载体同名 `i32`，但身份是 `acme::i32`。
///
/// 修复前字段按名字拿到 `Integer32` 载体（size 4）；修复后必须落在 `acme::i32`
/// 自己的零字段值类型布局（size 1），字段类型是名义布局而不是裸整数载体。
#[test]
fn same_name_user_struct_does_not_borrow_builtin_carrier() {
    let module = lower(
        r#"
namespace acme;

[primitive("acme::i32")]
structure i32 { }

class Holder {
    x: i32,
}
"#,
    );
    let field = holder_field(&module);
    assert!(
        matches!(&field.ty, NyarType::Nominal { spelling, .. } if spelling.as_str() == "acme::i32"),
        "字段类型必须保留名义布局身份，而不是名字反推出的内建载体: {field:?}"
    );
    assert!(layout(&module, "i32").fields.is_empty(), "零字段值类型的语义布局没有字段");
}

/// 标注漂移反例：声明名字是 `i32`，但 `primitive_identity` 是 `std::primitive::u64`。
///
/// 身份是唯一事实来源：字段必须是 u64 载体（size 8），而不是名字暗示的 i32（size 4）。
/// 修复前按名字登记 carrier 会得到 `Integer32`，这条断言能发现漂移。
#[test]
fn primitive_identity_overrides_matching_struct_name() {
    let declaration = nyar_types::ItemId::from_index(600).expect("test ItemId");
    let holder = nyar_types::ItemId::from_index(601).expect("test ItemId");
    let identity: Box<str> = Box::from("std::primitive::u64");
    let mut module = HirModule::default();
    module.structs.push(HirStruct {
        constructor_declaration: Some(declaration),
        name: Identifier::new("i32"),
        namespace: vec![Identifier::new("acme")],
        is_value_type: true,
        primitive_identity: Some(identity.clone()),
        ..Default::default()
    });
    module.structs.push(HirStruct {
        constructor_declaration: Some(holder),
        name: Identifier::new("Holder"),
        namespace: vec![Identifier::new("acme")],
        fields: vec![HirField {
            id: None,
            name: Identifier::new("x"),
            doc: Default::default(),
            ty: ValkyrieType::Nominal { declaration, arguments: Vec::new(), primitive_identity: Some(identity) },
            visibility: Default::default(),
            is_mutable: false,
        }],
        ..Default::default()
    });
    let mut diagnostics = Vec::new();
    let plan = compute_aggregate_layout_plan(&module, &mut diagnostics);
    let holder_layout = plan.layouts.iter().find(|layout| layout.name == "Holder").expect("Holder 布局");
    let field = &holder_layout.fields[0];
    assert_eq!(
        field.ty,
        NyarType::Integer64 { signed: false },
        "载体必须由 primitive_identity（u64）决定，而不是同名短名 i32: {field:?}"
    );
    assert!(diagnostics.is_empty(), "漂移是身份优先的合法声明，不应产出结构化诊断: {diagnostics:?}");
}

/// 正向回归：正确标注的数值声明仍按其身份得到内建载体（修复不得收紧这条）。
#[test]
fn annotated_numeric_declaration_still_maps_to_carrier() {
    let module = lower(
        r#"
namespace acme;

[primitive("std::primitive::u64")]
structure Wide { }

class Holder {
    x: Wide,
}
"#,
    );
    let field = holder_field(&module);
    assert_eq!(field.ty, NyarType::Integer64 { signed: false }, "u64 身份必须映射到 u64 载体: {field:?}");
}

/// `usize`/`isize` 不是数值形状表身份：表里必须没有它们，宽度由 std 别名结算。
#[test]
fn numeric_shape_table_excludes_pointer_width_aliases() {
    assert!(numeric_primitive_identity_shape("std::primitive::usize").is_none(), "usize 是指针宽度别名，不属于数值量化表");
    assert!(numeric_primitive_identity_shape("std::primitive::isize").is_none(), "isize 是指针宽度别名，不属于数值量化表");
    // f128 仍是 128 位形状，但 1.0 没有内建载体（只记录结论，不降精度）。
    assert!(
        matches!(numeric_primitive_identity_shape("std::primitive::f128"), Some(PrimitiveNumericShape::Float { bits: 128 })),
        "f128 形状保持 128 位，不得静默降到 64"
    );
}

/// 64 位目标验收：`usize`/`isize` 经 std 的指针宽度别名结算后，字段必须是 8 字节。
#[test]
fn pointer_width_aliases_settle_to_eight_bytes() {
    let module = lower(
        r#"
namespace std::primitive;

[primitive("std::primitive::u64")] structure u64 { }
[primitive("std::primitive::i64")] structure i64 { }

type usize = u64;
type isize = i64;

class Holder {
    a: usize,
    b: isize,
}
"#,
    );
    let layout = layout(&module, "Holder");
    let a = &layout.fields[0];
    let b = &layout.fields[1];
    assert_eq!(a.ty, NyarType::Integer64 { signed: false }, "64 位目标 usize 必须结算成 u64: {a:?}");
    assert_eq!(b.ty, NyarType::Integer64 { signed: true }, "64 位目标 isize 必须结算成 i64: {b:?}");
}

/// 无声明的数值短名不得借载体：`structure i32 { }` 没有 `[primitive]` 身份时，
/// 字段只能按自身名义布局，不能因为名字像 i32 就变成 4 字节整数。
#[test]
fn identity_less_numeric_declaration_does_not_borrow_numeric_carrier() {
    let declaration = nyar_types::ItemId::from_index(602).expect("test ItemId");
    let holder = nyar_types::ItemId::from_index(603).expect("test ItemId");
    let mut module = HirModule::default();
    module.structs.push(HirStruct {
        constructor_declaration: Some(declaration),
        name: Identifier::new("i32"),
        namespace: vec![Identifier::new("acme")],
        is_value_type: true,
        ..Default::default()
    });
    module.structs.push(HirStruct {
        constructor_declaration: Some(holder),
        name: Identifier::new("Holder"),
        namespace: vec![Identifier::new("acme")],
        fields: vec![HirField {
            id: None,
            name: Identifier::new("x"),
            doc: Default::default(),
            ty: ValkyrieType::Nominal { declaration, arguments: Vec::new(), primitive_identity: None },
            visibility: Default::default(),
            is_mutable: false,
        }],
        ..Default::default()
    });
    let mut diagnostics = Vec::new();
    let plan = compute_aggregate_layout_plan(&module, &mut diagnostics);
    let holder_layout = plan.layouts.iter().find(|layout| layout.name == "Holder").expect("Holder 布局");
    let field = &holder_layout.fields[0];
    assert!(matches!(field.ty, NyarType::Nominal { .. }), "无身份声明的数值短名不得经 spelling 桥取载体: {field:?}");
}

/// 编码名桥仍然有效：`Utf8Text`（无 `[primitive]` 身份）仍按编码名映射到 `Utf8`。
#[test]
fn identity_less_encoding_name_keeps_encoding_carrier() {
    let module = lower(
        r#"
namespace test;

[primitive("std::primitive::u8")] structure u8 { }

class Utf8Text {
    _repr: [u8],
}

class Holder {
    text: Utf8Text,
}
"#,
    );
    let field = holder_field(&module);
    assert_eq!(field.ty, NyarType::Utf8, "编码名 `Utf8Text` 仍走编码桥: {field:?}");
}

/// 反例（修前观测点）：伪造 `[primitive("std::primitive::usize")]` 声明不再得到 4 字节整数。
///
/// 修前该声明经数值形状表 + 拼写桥变成 `Integer32 { signed: true }`（4 字节）；
/// 修后 `usize` 不是内建数值身份，声明只能按自身名义布局结算。
#[test]
fn malformed_usize_primitive_declaration_is_not_a_numeric_carrier() {
    let module = lower(
        r#"
namespace std::primitive;

[primitive("std::primitive::usize")]
structure usize { }

class Holder {
    x: usize,
}
"#,
    );
    let field = holder_field(&module);
    assert_ne!(field.ty, NyarType::Integer32 { signed: true }, "usize 不得按拼写降成 i32: {field:?}");
}
