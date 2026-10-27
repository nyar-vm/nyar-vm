use crate::compiler::{
    hir::lowering::{
        diagnostics::{plan_source, plan_span},
        errors::{DiscriminatorFailure, HirError},
    },
    mir::{FlagsLayout, MirSumDeclaration, MirSumVariant},
    types::hir::{HirEnum, HirExpr, HirExprKind, HirLiteral, HirModule, HirVariant},
};

pub(crate) fn compute_nominal_declarations(
    module: &HirModule,
    nominal_offset: u32,
    diagnostics: &mut Vec<crate::compiler::mir::MirDiagnostic>,
) -> (Vec<MirSumDeclaration>, Vec<FlagsLayout>) {
    (collect_sum_declarations(module, nominal_offset, diagnostics), collect_flags_layouts(module))
}

pub(crate) fn collect_sum_declarations(
    module: &HirModule,
    nominal_offset: u32,
    diagnostics: &mut Vec<crate::compiler::mir::MirDiagnostic>,
) -> Vec<MirSumDeclaration> {
    let mut next_nominal = nominal_offset;
    let mut next_variant = 0u32;
    let mut layouts = module
        .enums
        .iter()
        .map(|enum_def| {
            let mut next_implicit = 0u32;
            let variants = enum_def
                .variants
                .iter()
                .map(|variant| {
                    let tag = resolve_sum_variant_tag(&enum_def.name, variant, &mut next_implicit)
                        .expect("enum discriminators must be validated before sum layout collection");
                    let id = nyar_types::VariantId::from_index(next_variant).expect("variant identity overflow");
                    next_variant += 1;
                    MirSumVariant {
                        id,
                        declaration: variant.declaration,
                        name: variant.name.to_string(),
                        tag,
                        fields: variant
                            .fields
                            .iter()
                            .filter_map(|field| {
                                let Some(id) = field.id
                                else {
                                    diagnostics
                                        .push(crate::compiler::mir::MirDiagnostic::UnresolvedFieldIdentity { field: field.name.to_string() });
                                    return None;
                                };
                                Some(crate::compiler::mir::MirField { id, name: field.name.to_string(), ty: field.ty.clone() })
                            })
                            .collect(),
                        result_type: variant.result_type.clone(),
                    }
                })
                .collect();
            let nominal = nyar_types::NominalInstanceId::from_index(next_nominal).expect("nominal identity overflow");
            next_nominal += 1;
            MirSumDeclaration {
                nominal,
                declaration: enum_def.declaration,
                name: enum_def.name.to_string(),
                is_unite: enum_def.is_unity,
                is_value_type: enum_def.is_value_type,
                generics: enum_def.generics.clone(),
                variants,
            }
        })
        .collect::<Vec<_>>();
    // Dependency packages may define `Result` / `Option` without copying the
    // HirEnum into the consuming module. Fine/Fail/Some still need nominal
    // sum metadata for SumNew / SumPayloadGet contract checks.
    for export in &module.imported_semantic_exports {
        for enum_def in &export.enums {
            let mut next_implicit = 0u32;
            let variants = enum_def
                .variants
                .iter()
                .map(|variant| {
                    let tag = resolve_sum_variant_tag(&enum_def.name, variant, &mut next_implicit)
                        .expect("enum discriminators must be validated before sum layout collection");
                    let id = nyar_types::VariantId::from_index(next_variant).expect("variant identity overflow");
                    next_variant += 1;
                    MirSumVariant {
                        id,
                        declaration: variant.declaration,
                        name: variant.name.to_string(),
                        tag,
                        fields: variant
                            .fields
                            .iter()
                            .filter_map(|field| {
                                let Some(id) = field.id
                                else {
                                    diagnostics
                                        .push(crate::compiler::mir::MirDiagnostic::UnresolvedFieldIdentity { field: field.name.to_string() });
                                    return None;
                                };
                                Some(crate::compiler::mir::MirField { id, name: field.name.to_string(), ty: field.ty.clone() })
                            })
                            .collect(),
                        result_type: variant.result_type.clone(),
                    }
                })
                .collect();
            let nominal = nyar_types::NominalInstanceId::from_index(next_nominal).expect("nominal identity overflow");
            next_nominal += 1;
            let declaration = MirSumDeclaration {
                nominal,
                declaration: enum_def.declaration,
                name: enum_def.name.to_string(),
                is_unite: enum_def.is_unity,
                is_value_type: enum_def.is_value_type,
                generics: enum_def.generics.clone(),
                variants,
            };
            if !layouts.iter().any(|prior| prior.name == declaration.name) {
                layouts.push(declaration);
            }
        }
    }
    layouts
}

fn integer_literal_u32(expr: &HirExpr) -> Option<u32> {
    match &expr.kind {
        HirExprKind::Literal(HirLiteral::Integer64(value)) if *value >= 0 => u32::try_from(*value).ok(),
        _ => None,
    }
}

/// Resolve the next variant tag for `unite` (`[tag(N)]` or declaration order) and
/// `enums` (`Variant = N` or auto-increment after the last assigned tag).
pub(crate) fn resolve_sum_variant_tag(
    owner: &crate::compiler::types::Identifier,
    variant: &HirVariant,
    next_implicit: &mut u32,
) -> Result<u32, HirError> {
    let tag = if let Some(discriminator) = &variant.discriminator {
        integer_literal_u32(discriminator).ok_or_else(|| HirError::InvalidDiscriminator {
            aggregate: owner.to_string(),
            failure: DiscriminatorFailure::NotNonNegativeInteger,
            variant: Some(variant.name.to_string()),
            value: None,
            source: plan_source(),
            span: plan_span(),
        })?
    }
    else {
        let tag = *next_implicit;
        *next_implicit = tag.saturating_add(1);
        tag
    };
    if variant.discriminator.is_some() {
        *next_implicit = tag.saturating_add(1);
    }
    Ok(tag)
}

pub(crate) fn validate_sum_type_discriminators(owner: &crate::compiler::types::Identifier, variants: &[HirVariant]) -> Result<(), HirError> {
    use std::collections::BTreeSet;

    let mut next_implicit = 0u32;
    let mut seen = BTreeSet::new();
    for variant in variants {
        let tag = resolve_sum_variant_tag(owner, variant, &mut next_implicit)?;
        if !seen.insert(tag) {
            return Err(HirError::InvalidDiscriminator {
                aggregate: owner.to_string(),
                failure: DiscriminatorFailure::DuplicateTag,
                variant: Some(variant.name.to_string()),
                value: Some(tag.to_string()),
                source: plan_source(),
                span: plan_span(),
            });
        }
    }
    Ok(())
}

#[doc(hidden)]
pub fn validate_enum_discriminators(module: &HirModule) -> Result<(), HirError> {
    for enum_def in &module.enums {
        validate_sum_type_discriminators(&enum_def.name, &enum_def.variants)?;
    }
    for export in &module.imported_semantic_exports {
        for enum_def in &export.enums {
            validate_sum_type_discriminators(&enum_def.name, &enum_def.variants)?;
        }
    }
    Ok(())
}

pub(crate) fn collect_flags_layouts(module: &HirModule) -> Vec<FlagsLayout> {
    module.flags.iter().map(|flags| FlagsLayout { name: flags.name.to_string() }).collect()
}
