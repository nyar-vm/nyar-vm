//! trait-bound 与 impl owner 类型辅助。

use crate::compiler::types::{
    Identifier, NamePath,
    hir::{HirDependencySemanticExport, HirEnum, HirFunction, HirStruct, HirTrait, HirVariant, ValkyrieType},
};

pub(crate) fn impl_nominal_type_name(ty: &ValkyrieType) -> Option<&str> {
    match ty {
        ValkyrieType::Array(_) => Some("Array"),
        ValkyrieType::Named(name) => Some(name.as_str()),
        ValkyrieType::Apply(base, _) => impl_nominal_type_name(base),
        ValkyrieType::Float32 => Some("f32"),
        ValkyrieType::Float64 => Some("f64"),
        ValkyrieType::Boolean => Some("bool"),
        ValkyrieType::Character => Some("char"),
        ValkyrieType::Integer8 { signed: true } => Some("i8"),
        ValkyrieType::Integer8 { signed: false } => Some("u8"),
        ValkyrieType::Integer16 { signed: true } => Some("i16"),
        ValkyrieType::Integer16 { signed: false } => Some("u16"),
        ValkyrieType::Integer32 { signed: true } => Some("i32"),
        ValkyrieType::Integer32 { signed: false } => Some("u32"),
        ValkyrieType::Integer64 { signed: true } => Some("i64"),
        ValkyrieType::Integer64 { signed: false } => Some("u64"),
        ValkyrieType::Integer128 { signed: true } => Some("i128"),
        ValkyrieType::Integer128 { signed: false } => Some("u128"),
        // Nominal 无表面名；必须经 `impl_owner_identifier` 按 declaration 反查。
        ValkyrieType::Nominal { .. } => None,
        _ => None,
    }
}

/// imply owner 短名：优先 declaration → 结构/sum 名；禁止把 Nominal 当无名丢弃。
pub(crate) fn impl_owner_identifier(module: &crate::compiler::types::hir::HirModule, ty: &ValkyrieType) -> Option<Identifier> {
    match ty {
        ValkyrieType::Nominal { declaration, .. } => {
            find_struct_name_by_declaration(module, *declaration).or_else(|| find_enum_name_by_declaration(module, *declaration))
        }
        ValkyrieType::Apply(base, _) => impl_owner_identifier(module, base),
        other => impl_nominal_type_name(other).map(Identifier::new),
    }
}

pub(crate) fn impl_owner_identifier_from_export(export: &HirDependencySemanticExport, ty: &ValkyrieType) -> Option<Identifier> {
    match ty {
        ValkyrieType::Nominal { declaration, .. } => export
            .structs
            .iter()
            .find(|item| item.constructor_declaration == Some(*declaration))
            .map(|item| item.name.clone())
            .or_else(|| export.enums.iter().find(|item| item.declaration == Some(*declaration)).map(|item| item.name.clone())),
        ValkyrieType::Apply(base, _) => impl_owner_identifier_from_export(export, base),
        other => impl_nominal_type_name(other).map(Identifier::new),
    }
}

pub(crate) fn find_struct_name_by_declaration(
    module: &crate::compiler::types::hir::HirModule,
    declaration: nyar_types::ItemId,
) -> Option<Identifier> {
    for item in &module.structs {
        if item.constructor_declaration == Some(declaration) {
            return Some(item.name.clone());
        }
    }
    for export in &module.imported_semantic_exports {
        for item in &export.structs {
            if item.constructor_declaration == Some(declaration) {
                return Some(item.name.clone());
            }
        }
    }
    for submodule in &module.submodules {
        if let Some(name) = find_struct_name_by_declaration(submodule, declaration) {
            return Some(name);
        }
    }
    None
}

pub(crate) fn find_enum_name_by_declaration(
    module: &crate::compiler::types::hir::HirModule,
    declaration: nyar_types::ItemId,
) -> Option<Identifier> {
    for item in &module.enums {
        if item.declaration == Some(declaration) {
            return Some(item.name.clone());
        }
    }
    for export in &module.imported_semantic_exports {
        for item in &export.enums {
            if item.declaration == Some(declaration) {
                return Some(item.name.clone());
            }
        }
    }
    for submodule in &module.submodules {
        if let Some(name) = find_enum_name_by_declaration(submodule, declaration) {
            return Some(name);
        }
    }
    None
}
