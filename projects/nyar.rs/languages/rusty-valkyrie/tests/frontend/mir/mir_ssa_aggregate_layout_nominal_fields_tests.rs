//! 声明层聚合布局必须递归解析名义字段：primitive 载体进结构容器，闭合泛型实例走
//! 与 canonical 实例层同口径的已冻结声明拼写；缺事实结构化失败，不再 panic。
//!
//! 修复前 `bytes: ArrayList<u8>` 与 `_repr: [u8]` 命中 `build_aggregate_layout`
//! 的严格 concretize panic（`concretize_type` 对 `ValkyrieType::Nominal` 无条件 Err）。

use nyar::NyarType;
use rusty_valkyrie::{
    Identifier, MirDiagnostic, MirLowerer, ValkyrieCompiler,
    compiler::emitter::pipeline::canonical_program_from_semantic_mir,
    compiler::types::hir::{HirField, HirModule, HirStruct},
    compute_aggregate_layout_plan,
    types::hir::types::ValkyrieType,
};

/// 数值原始类型不是语言内建，必须绑定到源码声明身份（与 `mir_branch_binding_merge_tests` 同族夹具）。
const PRIMITIVES: &str = r#"
namespace core::primitive;
[primitive("core::primitive::bool")] structure bool { }
[primitive("core::primitive::i32")] structure i32 { }
[primitive("core::primitive::u8")] structure u8 { }
[primitive("core::primitive::usize")] structure usize { }
"#;

/// `Utf8Builder.bytes: ArrayList<u8>`（闭合泛型实例）与 `Utf8Text._repr: [u8]`
///（结构容器内 primitive Nominal）即产品 std 的触发形状。
const NOMINAL_FIELD_SOURCE: &str = r#"
class ArrayList<T> {
    data: [T],
    size: usize,
}

class Utf8Builder {
    bytes: ArrayList<u8>,
    len: usize,
}

class Utf8Text {
    _repr: [u8],
}

micro probe(flag: bool) -> bool {
    return flag
}
"#;

fn compile(source: &str) -> rusty_valkyrie::compiler::mir::ssa::MirModule {
    let hir = ValkyrieCompiler::default()
        .compile_source(&format!("{PRIMITIVES}\n{source}"))
        .expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

/// 正向：含闭合泛型实例与 `[u8]` 字段的 class 不再 panic，产出解析后的实例级布局。
#[test]
fn nominal_generic_and_array_fields_lower_without_panic() {
    let module = compile(NOMINAL_FIELD_SOURCE);
    assert!(module.diagnostics.is_empty(), "闭合名义字段布局不得产生结构化失败: {:?}", module.diagnostics);

    let builder_layout = module.aggregate_layouts.layouts.iter().find(|layout| layout.name == "Utf8Builder").expect("Utf8Builder 布局");
    let builder = module.structs.iter().find(|aggregate| aggregate.name == "Utf8Builder").expect("Utf8Builder 声明");
    assert_eq!(
        module.aggregate_layouts.declaration_to_layout.get(&builder.declaration.expect("declaration identity")),
        Some(&builder_layout.id),
        "解析成功必须以已冻结 declaration 登记布局"
    );
    assert_eq!(
        builder_layout.fields[0].ty,
        NyarType::Apply(
            Box::new(NyarType::Named(Identifier::new("core::primitive::ArrayList"))),
            vec![NyarType::Integer8 { signed: false }]
        ),
        "ArrayList<u8> 必须按声明拼写解析为 Apply，实参经 primitive 载体代入"
    );
    assert_eq!(builder_layout.fields[0].size, 8, "引用语义泛型实例字段按引用槽计量");
    assert_eq!(builder_layout.fields[1].ty, NyarType::Integer32 { signed: true }, "usize 经已冻结拼写落到平台整数载体");

    let text_layout = module.aggregate_layouts.layouts.iter().find(|layout| layout.name == "Utf8Text").expect("Utf8Text 布局");
    assert_eq!(text_layout.fields[0].ty, NyarType::Array(Box::new(NyarType::Integer8 { signed: false })), "[u8] 元素必须递归映射到平台载体");

    // 开放泛型声明本身仍不得生成平台布局（字段仍是 binder）。
    assert!(!module.aggregate_layouts.layouts.iter().any(|layout| layout.name == "ArrayList"), "开放泛型声明不得进入声明级布局");
}

/// 正向：修复后的布局能继续通过 Canonical 合同（实例级字段身份与布局登记）。
#[test]
fn nominal_field_layout_fixture_reaches_canonical() {
    let module = compile(NOMINAL_FIELD_SOURCE);
    canonical_program_from_semantic_mir(&module).expect("声明级布局修复后 fixture 必须满足 Canonical 合同");
}

/// 负向：字段引用缺声明拼写（未知 declaration）时结构化失败，不 panic、不伪造登记。
#[test]
fn missing_declaration_spelling_yields_structured_diagnostic() {
    let mut module = HirModule::default();
    let holder = nyar_types::ItemId::from_index(500).expect("test ItemId");
    let ghost = nyar_types::ItemId::from_index(9_999).expect("test ItemId");
    module.structs.push(HirStruct {
        constructor_declaration: Some(holder),
        name: Identifier::new("GhostHolder"),
        namespace: vec![Identifier::new("test")],
        fields: vec![HirField {
            id: None,
            name: Identifier::new("ghost"),
            doc: Default::default(),
            ty: ValkyrieType::Nominal { declaration: ghost, arguments: Vec::new() },
            visibility: Default::default(),
            is_mutable: false,
        }],
        ..Default::default()
    });
    let mut diagnostics = Vec::new();
    let plan = compute_aggregate_layout_plan(&module, &mut diagnostics);
    assert!(
        diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MirDiagnostic::UnresolvedValueType { context } if context.contains("GhostHolder")
        )),
        "缺布局事实必须产出结构化诊断: {diagnostics:?}"
    );
    assert_eq!(plan.declaration_to_layout.get(&holder), None, "失败声明不得登记伪布局");
}

/// 负向：未代入 `Self` 字段同样结构化失败（旧路径 panic 的同类形状），不回退声明级短名。
#[test]
fn unsubstituted_self_field_yields_structured_diagnostic() {
    let mut module = HirModule::default();
    let holder = nyar_types::ItemId::from_index(501).expect("test ItemId");
    module.structs.push(HirStruct {
        constructor_declaration: Some(holder),
        name: Identifier::new("SelfHolder"),
        namespace: vec![Identifier::new("test")],
        fields: vec![HirField {
            id: None,
            name: Identifier::new("inner"),
            doc: Default::default(),
            ty: ValkyrieType::SelfType,
            visibility: Default::default(),
            is_mutable: false,
        }],
        ..Default::default()
    });
    let mut diagnostics = Vec::new();
    let plan = compute_aggregate_layout_plan(&module, &mut diagnostics);
    assert!(
        diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MirDiagnostic::UnresolvedValueType { context } if context.contains("SelfHolder")
        )),
        "未代入类型必须产出结构化诊断: {diagnostics:?}"
    );
    assert_eq!(plan.declaration_to_layout.get(&holder), None, "失败声明不得登记伪布局");
}
