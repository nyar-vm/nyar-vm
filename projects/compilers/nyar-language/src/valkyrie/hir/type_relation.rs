//! Unified parameter matching entry points for the current `HIR` layer.
//!
//! This module centralizes the settled matching categories used by overload
//! resolution and future call checking:
//! - nominal parameter matching
//! - named trait satisfaction
//! - anonymous row requirement satisfaction

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    hir::{
        nominal::{NominalModuleError, NominalModuleView},
        row::{RowMethodSignature, RowRequirement, RowRequirementError},
        trait_system::{NamedTraitWitness, TraitModuleError, TraitModuleView},
    },
    types::{
        Identifier,
        hir::{HirModule, HirStruct, HirWhereConstraint, RowType, TraitObject, ValkyrieType},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRelationDiagnosticSeed {
    IncompatibleTypes { actual: ValkyrieType, expected: ValkyrieType },
    UnsupportedActualType { actual: ValkyrieType },
    UnsupportedExpectedType { expected: ValkyrieType },
    Nominal { error: NominalModuleError },
    Trait { error: TraitModuleError },
    Row { errors: Vec<RowRequirementError> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterMatchResult {
    NominalExact,
    NominalSubtype { distance: usize },
    Trait { witness: NamedTraitWitness },
    Row,
    NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed },
}

#[derive(Debug, Clone)]
pub struct TypeRelationContext {
    nominal: NominalModuleView,
    traits: TraitModuleView,
    structs: BTreeMap<Identifier, HirStruct>,
    structs_by_declaration: BTreeMap<nyar_types::ItemId, HirStruct>,
    callable_constraints: Vec<HirWhereConstraint>,
}

impl TypeRelationContext {
    pub fn from_module(module: &HirModule) -> Self {
        let mut structs = BTreeMap::new();
        let mut structs_by_declaration = BTreeMap::new();
        collect_relation_structs(module, &mut structs, &mut structs_by_declaration);
        Self {
            nominal: NominalModuleView::from_module(module),
            traits: TraitModuleView::from_module(module),
            structs,
            structs_by_declaration,
            callable_constraints: Vec::new(),
        }
    }

    /// 短名 → 已冻结的 `Nominal{ItemId}`（有 declaration 时）；供 Self/AutoType 回收，禁止只写回 Named。
    pub fn nominal_type_for_name(&self, name: &Identifier) -> Option<ValkyrieType> {
        let item = self.structs.get(name)?;
        Some(ValkyrieType::Nominal { declaration: item.constructor_declaration?, arguments: Vec::new() })
    }

    /// 字面量特化用：`Named("usize")` 与对应 `Nominal{ItemId}` 都算数值类型。
    pub fn is_numeric_type(&self, ty: &ValkyrieType) -> bool {
        match ty {
            ValkyrieType::Nominal { declaration, .. } => self
                .structs_by_declaration
                .get(declaration)
                .is_some_and(|item| is_numeric_type_name(item.name.as_str())),
            other => is_numeric_type_name_of(other),
        }
    }

    pub fn with_callable_constraints(&self, constraints: &[HirWhereConstraint]) -> Self {
        let mut context = self.clone();
        context.callable_constraints = constraints.to_vec();
        context
    }

    pub fn callable_constraints(&self) -> &[HirWhereConstraint] {
        &self.callable_constraints
    }

    pub fn match_parameter(&self, actual: &ValkyrieType, expected: &ValkyrieType) -> ParameterMatchResult {
        if trivially_compatible(actual, expected) {
            return ParameterMatchResult::NominalExact;
        }

        // `Nominal{ItemId}` 只认 declaration；同 declaration + 实参相容即为 exact。
        if let (
            ValkyrieType::Nominal { declaration: actual_decl, arguments: actual_args },
            ValkyrieType::Nominal { declaration: expected_decl, arguments: expected_args },
        ) = (actual, expected)
        {
            if actual_decl == expected_decl
                && actual_args.len() == expected_args.len()
                && actual_args.iter().zip(expected_args.iter()).all(|(left, right)| self.matches_non_error(left, right))
            {
                return ParameterMatchResult::NominalExact;
            }
        }

        // Nominal applications retain their type arguments at the semantic
        // boundary.  Match the constructor nominally and each argument using
        // the same parameter relation, so exported `Choice<T, E>` can accept
        // a consumer-side `Choice<i32, utf8>` without erasing either generic
        // argument or falling back to a backend representation.
        if let (ValkyrieType::Apply(actual_base, actual_args), ValkyrieType::Apply(expected_base, expected_args)) = (actual, expected) {
            if actual_args.len() == expected_args.len()
                && matches!(
                    self.match_parameter(actual_base, expected_base),
                    ParameterMatchResult::NominalExact | ParameterMatchResult::NominalSubtype { .. }
                )
                && actual_args.iter().zip(expected_args).all(|(actual_arg, expected_arg)| {
                    !matches!(self.match_parameter(actual_arg, expected_arg), ParameterMatchResult::NoMatch { .. })
                })
            {
                return ParameterMatchResult::NominalExact;
            }
        }

        match expected {
            ValkyrieType::Union(expected_items) => {
                if expected_items.iter().any(|item| self.matches_non_error(actual, item)) {
                    return ParameterMatchResult::NominalExact;
                }
                if let ValkyrieType::Union(actual_items) = actual {
                    let all_assignable = actual_items
                        .iter()
                        .all(|actual_item| expected_items.iter().any(|expected_item| self.matches_non_error(actual_item, expected_item)));
                    if all_assignable {
                        return ParameterMatchResult::NominalExact;
                    }
                }
            }
            ValkyrieType::Intersection(expected_items) => {
                let all_assignable = expected_items.iter().all(|item| self.matches_non_error(actual, item));
                if all_assignable {
                    return ParameterMatchResult::NominalExact;
                }
            }
            ValkyrieType::Named(expected_name) => {
                if let Some(actual_name) = named_type_name(actual) {
                    let nominal = self.match_nominal_parameter(&actual_name, expected_name);
                    match &nominal {
                        ParameterMatchResult::NominalExact | ParameterMatchResult::NominalSubtype { .. } => return nominal,
                        ParameterMatchResult::NoMatch {
                            diagnostic_seed: TypeRelationDiagnosticSeed::Nominal { error: NominalModuleError::UnknownType { .. } },
                        } => {
                            if let Some(result) = self.match_named_trait_parameter(&actual_name, expected_name) {
                                return result;
                            }
                        }
                        ParameterMatchResult::NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed::Nominal { .. } } => return nominal,
                        ParameterMatchResult::NoMatch { .. } => {
                            if let Some(result) = self.match_named_trait_parameter(&actual_name, expected_name) {
                                return result;
                            }
                            return nominal;
                        }
                        ParameterMatchResult::Trait { .. } | ParameterMatchResult::Row => {
                            unreachable!("nominal matching should not yield trait/row")
                        }
                    }
                }
            }
            ValkyrieType::TraitObject(TraitObject { trait_path, .. }) => {
                if let Some(actual_name) = named_type_name(actual) {
                    return self.match_named_trait_parameter(&actual_name, trait_path).unwrap_or(ParameterMatchResult::NoMatch {
                        diagnostic_seed: TypeRelationDiagnosticSeed::IncompatibleTypes { actual: actual.clone(), expected: expected.clone() },
                    });
                }
            }
            ValkyrieType::Row(row) => {
                if let Some(actual_name) = named_type_name(actual) {
                    return self.match_row_requirement(&actual_name, &row_type_to_requirement(row));
                }
            }
            _ => {}
        }

        ParameterMatchResult::NoMatch {
            diagnostic_seed: TypeRelationDiagnosticSeed::IncompatibleTypes { actual: actual.clone(), expected: expected.clone() },
        }
    }

    pub fn match_nominal_parameter(&self, actual_name: &Identifier, expected_name: &Identifier) -> ParameterMatchResult {
        match self.nominal.nominal_match_distance(actual_name, expected_name) {
            Ok(Some(0)) => ParameterMatchResult::NominalExact,
            Ok(Some(distance)) => ParameterMatchResult::NominalSubtype { distance },
            Ok(None) => ParameterMatchResult::NoMatch {
                diagnostic_seed: TypeRelationDiagnosticSeed::IncompatibleTypes {
                    actual: ValkyrieType::Named(actual_name.clone()),
                    expected: ValkyrieType::Named(expected_name.clone()),
                },
            },
            Err(error) => ParameterMatchResult::NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed::Nominal { error } },
        }
    }

    pub fn match_named_trait_parameter(&self, actual_name: &Identifier, trait_name: &Identifier) -> Option<ParameterMatchResult> {
        match self.traits.satisfy_named_trait(actual_name, trait_name) {
            Ok(witness) => Some(ParameterMatchResult::Trait { witness }),
            Err(TraitModuleError::UnknownTrait { .. }) => None,
            Err(error) => Some(ParameterMatchResult::NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed::Trait { error } }),
        }
    }

    /// 在泛型实参完成代入后验证 callable 的声明级 where 合同。
    pub fn satisfies_where_constraints(&self, constraints: &[HirWhereConstraint], substitutions: &BTreeMap<Identifier, ValkyrieType>) -> bool {
        constraints.iter().all(|constraint| {
            let target = substitute_relation_type(&constraint.target, substitutions);
            constraint.bounds.iter().all(|bound| {
                if let Some(context) = self.callable_constraints.iter().find(|context| {
                    let context_target = substitute_relation_type(&context.target, substitutions);
                    context_target == target
                }) {
                    return bound_is_satisfied_by_context(bound, context, &self.callable_constraints, substitutions);
                }
                let Ok(witness) = self.traits.satisfy_explicit_type(&target, bound) else { return false };
                bound.associated_types.iter().all(|equation| {
                    let expected = substitute_relation_type(&equation.ty, substitutions);
                    witness.associated_types.get(&equation.name).is_some_and(|actual| actual == &expected)
                })
            })
        })
    }

    pub fn match_row_requirement(&self, actual_name: &Identifier, requirement: &RowRequirement) -> ParameterMatchResult {
        let Some(candidate) = self.structs.get(actual_name)
        else {
            return ParameterMatchResult::NoMatch {
                diagnostic_seed: TypeRelationDiagnosticSeed::UnsupportedActualType { actual: ValkyrieType::Named(actual_name.clone()) },
            };
        };

        match requirement.check_struct(candidate) {
            Ok(()) => ParameterMatchResult::Row,
            Err(errors) => ParameterMatchResult::NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed::Row { errors } },
        }
    }

    fn matches_non_error(&self, actual: &ValkyrieType, expected: &ValkyrieType) -> bool {
        !matches!(self.match_parameter(actual, expected), ParameterMatchResult::NoMatch { .. })
    }

}

fn collect_relation_structs(
    module: &HirModule,
    by_name: &mut BTreeMap<Identifier, HirStruct>,
    by_declaration: &mut BTreeMap<nyar_types::ItemId, HirStruct>,
) {
    for item in &module.structs {
        by_name.entry(item.name.clone()).or_insert_with(|| item.clone());
        if let Some(declaration) = item.constructor_declaration {
            by_declaration.entry(declaration).or_insert_with(|| item.clone());
        }
    }
    for export in &module.imported_semantic_exports {
        for item in &export.structs {
            by_name.entry(item.name.clone()).or_insert_with(|| item.clone());
            if let Some(declaration) = item.constructor_declaration {
                by_declaration.entry(declaration).or_insert_with(|| item.clone());
            }
        }
    }
    for submodule in &module.submodules {
        collect_relation_structs(submodule, by_name, by_declaration);
    }
}

fn is_numeric_type_name(name: &str) -> bool {
    matches!(
        name,
        "byte"
            | "sbyte"
            | "usize"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "f32"
            | "f64"
            | "f128"
            | "core.primitive.usize"
            | "core.primitive.isize"
            | "core::primitive::usize"
            | "core::primitive::isize"
    )
}

fn is_numeric_type_name_of(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Integer8 { .. }
        | ValkyrieType::Integer16 { .. }
        | ValkyrieType::Integer32 { .. }
        | ValkyrieType::Integer64 { .. }
        | ValkyrieType::Integer128 { .. }
        | ValkyrieType::Float32
        | ValkyrieType::Float64 => true,
        ValkyrieType::Named(name) => is_numeric_type_name(name.as_str()),
        _ => false,
    }
}

fn bound_is_satisfied_by_context(
    required: &crate::types::hir::HirTraitBound,
    context: &HirWhereConstraint,
    all_contexts: &[HirWhereConstraint],
    substitutions: &BTreeMap<Identifier, ValkyrieType>,
) -> bool {
    context.bounds.iter().any(|provided| {
        provided.declaration.is_some() && provided.declaration == required.declaration
            && provided.type_arguments.len() == required.type_arguments.len()
            && provided.type_arguments.iter().zip(&required.type_arguments).all(|(provided, required)| {
                normalize_context_type(provided, all_contexts, substitutions) == normalize_context_type(required, all_contexts, substitutions)
            })
            && required.associated_types.iter().all(|required_equation| {
                let expected = normalize_context_type(&required_equation.ty, all_contexts, substitutions);
                provided.associated_types.iter().any(|provided_equation| {
                    provided_equation.name == required_equation.name
                        && normalize_context_type(&provided_equation.ty, all_contexts, substitutions) == expected
                })
            })
    })
}

fn normalize_context_type(
    ty: &ValkyrieType,
    contexts: &[HirWhereConstraint],
    substitutions: &BTreeMap<Identifier, ValkyrieType>,
) -> ValkyrieType {
    let ty = substitute_relation_type(ty, substitutions);
    let ValkyrieType::Associated(associated) = &ty
    else {
        return ty;
    };
    let Some(base) = named_type_name(&associated.base)
    else {
        return ty;
    };
    contexts
        .iter()
        .filter(|context| named_type_name(&substitute_relation_type(&context.target, substitutions)).is_some_and(|target| target == base))
        .flat_map(|context| &context.bounds)
        .flat_map(|bound| &bound.associated_types)
        .find(|equation| equation.name == associated.name)
        .map(|equation| normalize_context_type(&equation.ty, contexts, substitutions))
        .unwrap_or(ty)
}

fn substitute_relation_type(ty: &ValkyrieType, substitutions: &BTreeMap<Identifier, ValkyrieType>) -> ValkyrieType {
    match ty {
        ValkyrieType::Named(name) => substitutions.get(name).cloned().unwrap_or_else(|| ty.clone()),
        ValkyrieType::Generic(generic) => substitutions.get(&generic.name).cloned().unwrap_or_else(|| ty.clone()),
        ValkyrieType::Apply(base, args) => ValkyrieType::Apply(
            Box::new(substitute_relation_type(base, substitutions)),
            args.iter().map(|arg| substitute_relation_type(arg, substitutions)).collect(),
        ),
        ValkyrieType::Array(inner) => ValkyrieType::Array(Box::new(substitute_relation_type(inner, substitutions))),
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(items.iter().map(|item| substitute_relation_type(item, substitutions)).collect()),
        ValkyrieType::Union(items) => ValkyrieType::Union(items.iter().map(|item| substitute_relation_type(item, substitutions)).collect()),
        other => other.clone(),
    }
}

fn row_type_to_requirement(row: &RowType) -> RowRequirement {
    RowRequirement::from_methods(
        row.methods
            .iter()
            .map(|method| RowMethodSignature {
                name: method.name.clone(),
                params: method.params.clone(),
                return_type: method.return_type.clone(),
            })
            .collect(),
    )
}

fn named_type_name(ty: &ValkyrieType) -> Option<Identifier> {
    match ty {
        ValkyrieType::Named(name) => Some(name.clone()),
        ValkyrieType::Generic(generic) => Some(generic.name.clone()),
        ValkyrieType::Apply(base, _) => named_type_name(base),
        _ => None,
    }
}

fn trivially_compatible(actual: &ValkyrieType, expected: &ValkyrieType) -> bool {
    match (actual, expected) {
        // `Self` is only a placeholder inside an imply body; overload matching must
        // substitute it with the owner before calling this (see `substitute_self_type`).
        // Keep identity for unresolved Self-vs-Self only — never "Self matches anything".
        (ValkyrieType::r#SelfType, ValkyrieType::r#SelfType) => true,
        (ValkyrieType::Array(actual), ValkyrieType::Array(expected)) => trivially_compatible(actual, expected),
        (ValkyrieType::Tuple(actual), ValkyrieType::Tuple(expected)) if actual.len() == expected.len() => {
            actual.iter().zip(expected).all(|(actual, expected)| trivially_compatible(actual, expected))
        }
        (ValkyrieType::Intersection(actual), ValkyrieType::Intersection(expected)) if actual.len() == expected.len() => {
            actual.iter().zip(expected).all(|(actual, expected)| trivially_compatible(actual, expected))
        }
        _ => {
            actual == expected
                || matches!(actual, ValkyrieType::AutoType)
                || matches!(expected, ValkyrieType::AutoType)
        }
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn unresolved_primitive_and_array_spellings_do_not_create_type_equivalence() {
        let context = TypeRelationContext::from_module(&HirModule::default());
        for (spelling, actual) in [
            ("f64", ValkyrieType::Float64),
            ("core::primitive::f64", ValkyrieType::Float64),
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
        let hir = crate::ValkyrieCompiler::default()
            .compile_source("structure Record { value: i32 }")
            .expect("结构声明解析");
        let context = TypeRelationContext::from_module(&hir);
        let item = hir.structs.first().expect("结构声明");
        let nominal = ValkyrieType::Nominal {
            declaration: item.constructor_declaration.expect("冻结声明身份"),
            arguments: vec![],
        };
        let unresolved = ValkyrieType::Named(item.name.clone());
        assert!(matches!(context.match_parameter(&nominal, &unresolved), ParameterMatchResult::NoMatch { .. }));
        assert!(matches!(context.match_parameter(&unresolved, &nominal), ParameterMatchResult::NoMatch { .. }));
        assert_eq!(context.match_parameter(&nominal, &nominal), ParameterMatchResult::NominalExact);
    }
}
