//! 属性修饰符与 setter 校验检查。

use crate::types::{
    Identifier, SourceSpan,
    hir::{HirModule, HirProperty, HirStruct},
};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyErrorKind {
    VirtualStaticConflict { property: Identifier },
    StaticWithSelf { property: Identifier },
    LazyPropertyWithSetter { property: Identifier },
    InvalidOverride { class_name: Identifier, property: Identifier },
    AbstractPropertyWithBody { class_name: Identifier, property: Identifier },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyError {
    pub kind: PropertyErrorKind,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl PropertyError {
    pub fn virtual_static_conflict(property: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: PropertyErrorKind::VirtualStaticConflict { property: property.clone() },
            message: format!("property {} cannot be both virtual and static", property),
            span,
        }
    }

    pub fn static_with_self(property: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: PropertyErrorKind::StaticWithSelf { property: property.clone() },
            message: format!("static property {} cannot use self", property),
            span,
        }
    }

    pub fn lazy_with_setter(property: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: PropertyErrorKind::LazyPropertyWithSetter { property: property.clone() },
            message: format!("lazy property {} cannot define a setter", property),
            span,
        }
    }

    pub fn invalid_override(class_name: Identifier, property: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: PropertyErrorKind::InvalidOverride { class_name: class_name.clone(), property: property.clone() },
            message: format!("property {} in {} cannot override without a parent", property, class_name),
            span,
        }
    }

    pub fn abstract_property_with_body(class_name: Identifier, property: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: PropertyErrorKind::AbstractPropertyWithBody { class_name: class_name.clone(), property: property.clone() },
            message: format!("abstract property {} in {} cannot have a concrete body", property, class_name),
            span,
        }
    }
}

impl fmt::Display for PropertyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PropertyError {}

#[derive(Debug, Default)]
pub struct PropertyChecker {
    errors: Vec<PropertyError>,
}

impl PropertyChecker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check_module(&mut self, module: &HirModule) -> Vec<PropertyError> {
        self.errors.clear();
        for class in &module.structs {
            for property in &class.properties {
                self.check_property(class, property);
            }
        }
        self.errors.clone()
    }

    fn check_property(&mut self, class: &HirStruct, property: &HirProperty) {
        if property.is_static && property.is_virtual {
            self.errors.push(PropertyError::virtual_static_conflict(property.name.clone(), None));
        }
        if property.is_lazy && property.setter.is_some() {
            self.errors.push(PropertyError::lazy_with_setter(property.name.clone(), None));
        }
        if property.is_override && class.parents.is_empty() {
            self.errors.push(PropertyError::invalid_override(class.name.clone(), property.name.clone(), None));
        }
        if property.is_abstract && !class.is_abstract {
            self.errors.push(PropertyError::abstract_property_with_body(class.name.clone(), property.name.clone(), None));
        }
    }
}

#[derive(Debug, Default)]
pub struct SetterValidationAnalyzer;

impl SetterValidationAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze(&mut self, property: &HirProperty) -> Vec<String> {
        vec![format!("validate setter for {}", property.name)]
    }

    pub fn generate_panic_code(property_name: &str, condition: &str) -> String {
        format!("if !({}) {{ panic!(\"invalid setter value for {}\"); }}", condition, property_name)
    }
}
