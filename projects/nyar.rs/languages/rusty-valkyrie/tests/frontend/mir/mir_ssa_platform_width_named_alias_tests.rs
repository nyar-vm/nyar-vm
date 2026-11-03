//! 指针宽度整数面在平台 concretize 与声明聚合布局中的失败关闭。
//!
//! `usize`/`isize` 的宽度由目标决定，必须由 `std::primitive::pointer_width` 的别名
//! 展开结算成定宽整数。若 `Named("usize")` 存活到平台层，旧实现在
//! `concretize_primitive_named_alias` 里静默给出 `Integer32`（4 字节载体），
//! 64 位目标上字段布局错误且没有任何诊断。这里锁定结构化失败：错误含类型名，
//! 聚合诊断含声明位置与字段名，失败声明不登记布局。

use rusty_valkyrie::{
    Identifier, MirDiagnostic, compute_aggregate_layout_plan,
    compiler::{
        emitter::frontend_contract::concretize_type,
        types::hir::{HirField, HirModule, HirStruct},
    },
    types::hir::types::ValkyrieType,
};

fn named_module(item_index: u32, field_type: &str) -> (nyar_types::ItemId, HirModule) {
    let holder = nyar_types::ItemId::from_index(item_index).expect("test ItemId");
    let mut module = HirModule::default();
    module.structs.push(HirStruct {
        constructor_declaration: Some(holder),
        name: Identifier::new("PointerHolder"),
        namespace: vec![Identifier::new("test")],
        fields: vec![HirField {
            id: None,
            name: Identifier::new("cursor"),
            doc: Default::default(),
            ty: ValkyrieType::Named(Identifier::new(field_type)),
            visibility: Default::default(),
            is_mutable: false,
        }],
        ..Default::default()
    });
    (holder, module)
}

/// 反例：`usize`/`isize`（含限定拼写）存活到 concretize 时必须报错，不得猜 32 位。
#[test]
fn platform_width_named_spellings_fail_with_type_name() {
    for name in ["usize", "isize", "std::primitive::usize", "std::primitive::isize"] {
        let error = concretize_type(&ValkyrieType::Named(Identifier::new(name)))
            .expect_err("指针宽度面缺别名展开必须失败关闭");
        assert!(error.message.contains(name), "结构化错误必须含类型名 `{name}`: {error}");
        assert!(!error.message.contains("Integer32"), "不得静默给出 32 位载体: {error}");
    }
}

/// 反例：`Named("usize")` 字段让整个声明布局失败，诊断含聚合拼写、字段名与类型名。
#[test]
fn named_platform_width_field_yields_structured_diagnostic() {
    for (index, field_type) in [(700, "usize"), (701, "isize")] {
        let (holder, module) = named_module(index, field_type);
        let mut diagnostics = Vec::new();
        let plan = compute_aggregate_layout_plan(&module, &mut diagnostics);
        assert_eq!(plan.declaration_to_layout.get(&holder), None, "失败关闭不得按名字伪造布局");
        assert!(
            !plan.layouts.iter().any(|layout| layout.name == "PointerHolder"),
            "失败声明不得进入布局表: {:?}",
            plan.layouts
        );
        assert!(
            diagnostics.iter().any(|diagnostic| matches!(
                diagnostic,
                MirDiagnostic::UnresolvedValueType { context }
                    if context.contains("PointerHolder") && context.contains("cursor") && context.contains(field_type)
            )),
            "诊断必须含声明位置、字段名与类型名: {diagnostics:?}"
        );
    }
}

/// 正向守卫：定宽拼写仍按声明拼写给载体，失败关闭不得误伤。
#[test]
fn fixed_width_spellings_keep_their_carriers() {
    assert_eq!(concretize_type(&ValkyrieType::Named(Identifier::new("u32"))), Ok(nyar::NyarType::Integer32 { signed: true }));
    assert_eq!(concretize_type(&ValkyrieType::Named(Identifier::new("u64"))), Ok(nyar::NyarType::Integer64 { signed: true }));
    assert_eq!(concretize_type(&ValkyrieType::Named(Identifier::new("f32"))), Ok(nyar::NyarType::Float32));
}
