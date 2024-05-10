//! 赋值、传参与返回的复制语义合同。

use crate::types::{
    Identifier,
    hir::{HirField, HirStruct, ValkyrieType as HirType},
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentSemantics {
    Copy,
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterSemantics {
    Copy,
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnSemantics {
    Copy,
    Reference,
}

#[derive(Debug, Default)]
pub struct CopySemanticsValidator {
    value_types: BTreeSet<Identifier>,
    value_type_declarations: BTreeSet<nyar_types::ItemId>,
}

/// HIR 层仍可能以 `Named("usize")` 等形式出现的标量别名；它们与整数/浮点一样可拷贝。
pub(super) fn is_copy_compatible_primitive_alias(name: &str) -> bool {
    matches!(
        name,
        "usize"
            | "isize"
            | "byte"
            | "sbyte"
            | "u8"
            | "i8"
            | "u16"
            | "i16"
            | "u32"
            | "i32"
            | "u64"
            | "i64"
            | "u128"
            | "i128"
            | "f32"
            | "f64"
            | "f128"
            | "bool"
            | "char"
            | "utf8"
            | "utf16"
    )
}

impl CopySemanticsValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_value_type(&mut self, class: &HirStruct) {
        if class.is_value_type {
            self.value_types.insert(class.name.clone());
            if let Some(declaration) = class.constructor_declaration {
                self.value_type_declarations.insert(declaration);
            }
        }
    }

    fn is_value(&self, ty: &HirType) -> bool {
        match ty {
            HirType::Named(name) => is_copy_compatible_primitive_alias(name.as_str()) || self.value_types.contains(name),
            HirType::Nominal { declaration, .. } => self.value_type_declarations.contains(declaration),
            HirType::Tuple(_) | HirType::FixedArray { .. } => true,
            _ => false,
        }
    }

    pub fn validate_assignment(&self, ty: &HirType) -> AssignmentSemantics {
        if self.is_value(ty) { AssignmentSemantics::Copy } else { AssignmentSemantics::Reference }
    }

    pub fn validate_parameter_passing(&self, ty: &HirType) -> ParameterSemantics {
        if self.is_value(ty) { ParameterSemantics::Copy } else { ParameterSemantics::Reference }
    }

    pub fn validate_return(&self, ty: &HirType) -> ReturnSemantics {
        if self.is_value(ty) { ReturnSemantics::Copy } else { ReturnSemantics::Reference }
    }

    /// 判断给定类型是否为已注册的值类型或内建值语义类型（元组、定长数组）。
    pub fn is_value_type(&self, ty: &HirType) -> bool {
        self.is_value(ty)
    }

    /// 判断给定类型是否可被值类型安全地按拷贝语义持有。
    ///
    /// 仅 copy 兼容类型可作为值类型 struct 的字段：基本标量、定长数组、元组、
    /// 以及已注册的值类型。堆数组、函数、trait 对象、未注册命名类型等引用类型
    /// 不兼容，违反 copy discipline。
    fn is_copy_compatible(&self, ty: &HirType) -> bool {
        match ty {
            HirType::Void
            | HirType::Unit
            | HirType::Boolean
            | HirType::Character
            | HirType::Utf8
            | HirType::Utf16
            | HirType::Float32
            | HirType::Float64
            | HirType::AutoType
            | HirType::SelfType => true,
            HirType::Integer8 { .. }
            | HirType::Integer16 { .. }
            | HirType::Integer32 { .. }
            | HirType::Integer64 { .. }
            | HirType::Integer128 { .. } => true,
            HirType::Tuple(items) => items.iter().all(|item| self.is_copy_compatible(item)),
            HirType::FixedArray { element, .. } => self.is_copy_compatible(element),
            HirType::Named(name) => is_copy_compatible_primitive_alias(name.as_str()) || self.value_types.contains(name),
            // 已绑定 declaration：按声明侧值类型登记判断，禁止再按短名回查。
            HirType::Nominal { declaration, .. } => self.value_type_declarations.contains(declaration),
            HirType::Array(_)
            | HirType::Function(_)
            | HirType::TypeLambda(_)
            | HirType::TraitObject(_)
            | HirType::Associated(_)
            | HirType::Generic(_)
            | HirType::Row(_)
            | HirType::Apply(_, _)
            | HirType::Nullable(_)
            | HirType::Union(_)
            | HirType::Intersection(_) => false,
        }
    }

    /// 在值类型结构体中查找首个引用类型字段，返回该字段引用以便定位违规。
    ///
    /// 值类型应当只包含拷贝兼容的字段；若存在堆数组或未注册命名类型等引用字段，
    /// 则该值类型无法被安全地按拷贝语义传递或返回，违反 copy discipline。
    pub fn find_non_copy_field<'a>(&self, struct_def: &'a HirStruct) -> Option<&'a HirField> {
        struct_def.fields.iter().find(|field| !self.is_copy_compatible(&field.ty))
    }
}
