//! sealed class、enum 注册表与穷尽性检查。

use crate::types::{
    Identifier, SourceSpan,
    hir::{HirModule, HirStruct},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealedClassErrorKind {
    NonExhaustiveMatch,
    UnknownSealedClass,
    NotSealedBase,
    DuplicateMatchArm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedClassError {
    pub kind: SealedClassErrorKind,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl fmt::Display for SealedClassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SealedClassError {}

#[derive(Debug, Default, Clone)]
pub struct SealedClassRegistry {
    sealed_classes: BTreeSet<Identifier>,
    subclasses: BTreeMap<Identifier, BTreeSet<Identifier>>,
}

impl SealedClassRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_sealed_class(&mut self, class: &HirStruct) {
        if class.is_sealed {
            self.sealed_classes.insert(class.name.clone());
        }
    }

    pub fn register_subclass(&mut self, sealed_name: &Identifier, subclass: &Identifier) -> Result<(), SealedClassError> {
        if !self.sealed_classes.contains(sealed_name) {
            return Err(SealedClassError {
                kind: SealedClassErrorKind::NotSealedBase,
                message: format!("{} is not a sealed class", sealed_name),
                span: None,
            });
        }
        self.subclasses.entry(sealed_name.clone()).or_default().insert(subclass.clone());
        Ok(())
    }

    pub fn is_sealed_class(&self, name: &Identifier) -> bool {
        self.sealed_classes.contains(name)
    }

    pub fn get_permitted_subclasses(&self, name: &Identifier) -> Vec<Identifier> {
        self.subclasses.get(name).map(|items| items.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn sealed_class_names(&self) -> impl Iterator<Item = &Identifier> {
        self.sealed_classes.iter()
    }
}

#[derive(Debug, Default, Clone)]
pub struct EnumRegistry {
    sum_types: BTreeMap<Identifier, Vec<Identifier>>,
}

impl EnumRegistry {
    pub fn from_module(module: &HirModule) -> Self {
        let mut registry = Self::default();
        for enum_def in module.enums.iter().chain(module.imported_enums.iter()).chain(module.imported_nominal_enums()) {
            registry.sum_types.insert(enum_def.name.clone(), enum_def.variants.iter().map(|variant| variant.name.clone()).collect());
        }
        registry
    }

    pub fn is_sum_type(&self, name: &Identifier) -> bool {
        self.sum_types.contains_key(name)
    }

    pub fn variants(&self, name: &Identifier) -> &[Identifier] {
        self.sum_types.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn enum_from_covered_variants(&self, covered: &[Identifier]) -> Option<Identifier> {
        let mut candidates = BTreeSet::new();
        for (enum_name, variants) in &self.sum_types {
            if covered.iter().any(|variant| variants.contains(variant)) {
                candidates.insert(enum_name.clone());
            }
        }
        if candidates.len() == 1 { candidates.into_iter().next() } else { None }
    }
}

#[derive(Debug, Clone)]
pub struct ExhaustivenessChecker {
    registry: SealedClassRegistry,
    enum_registry: EnumRegistry,
}

impl ExhaustivenessChecker {
    pub fn new(registry: SealedClassRegistry) -> Self {
        Self { registry, enum_registry: EnumRegistry::default() }
    }

    pub fn with_enum_registry(registry: SealedClassRegistry, enum_registry: EnumRegistry) -> Self {
        Self { registry, enum_registry }
    }

    pub fn enum_registry(&self) -> &EnumRegistry {
        &self.enum_registry
    }

    pub fn sealed_registry(&self) -> &SealedClassRegistry {
        &self.registry
    }

    pub fn check_exhaustiveness(&self, sealed_name: &Identifier, covered: &[Identifier]) -> Result<(), SealedClassError> {
        let declared = self.registry.get_permitted_subclasses(sealed_name);
        let covered_set = covered.iter().cloned().collect::<BTreeSet<_>>();
        let missing = declared.into_iter().filter(|name| !covered_set.contains(name)).collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        }
        else {
            Err(SealedClassError {
                kind: SealedClassErrorKind::NonExhaustiveMatch,
                message: format!(
                    "non exhaustive match for {}: missing {}",
                    sealed_name,
                    missing.iter().map(|item| item.to_string()).collect::<Vec<_>>().join(", ")
                ),
                span: None,
            })
        }
    }

    pub fn is_wildcard_exhaustive(&self, _type_name: &Identifier, has_wildcard: bool) -> bool {
        has_wildcard
    }

    pub fn check_duplicate_arms(&self, covered: &[Identifier]) -> Option<SealedClassError> {
        let mut seen = BTreeSet::new();
        for variant in covered {
            if !seen.insert(variant.clone()) {
                return Some(SealedClassError {
                    kind: SealedClassErrorKind::DuplicateMatchArm,
                    message: format!("duplicate match arm for {}", variant),
                    span: None,
                });
            }
        }
        None
    }

    pub fn check_variant_exhaustiveness(&self, enum_name: &Identifier, covered: &[Identifier]) -> Result<(), SealedClassError> {
        let declared = self.enum_registry.variants(enum_name);
        let covered_set = covered.iter().cloned().collect::<BTreeSet<_>>();
        let missing = declared.iter().filter(|variant| !covered_set.contains(*variant)).cloned().collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        }
        else {
            Err(SealedClassError {
                kind: SealedClassErrorKind::NonExhaustiveMatch,
                message: format!(
                    "non exhaustive match for {}: missing {}",
                    enum_name,
                    missing.iter().map(|item| item.to_string()).collect::<Vec<_>>().join(", ")
                ),
                span: None,
            })
        }
    }
}
