//! 赋值、传参与返回的复制语义合同。

use crate::compiler::types::{
    Identifier,
    hir::{HirField, HirStruct, ValkyrieType as HirType},
};
use std::collections::BTreeSet;

/// 赋值时采用的语义：按值拷贝还是按引用共享。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentSemantics {
    /// 按值拷贝。
    Copy,
    /// 按引用共享。
    Reference,
}

/// 传参时采用的语义：按值拷贝还是按引用共享。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterSemantics {
    /// 按值拷贝。
    Copy,
    /// 按引用共享。
    Reference,
}

/// 返回时采用的语义：按值拷贝还是按引用共享。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnSemantics {
    /// 按值拷贝。
    Copy,
    /// 按引用共享。
    Reference,
}

/// 依据已注册值类型判定赋值/传参/返回复制语义的校验器。
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
    /// 创建空的复制语义校验器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一个值类型 struct，并记录其构造器 declaration 身份。
    pub fn register_value_type(&mut self, class: &HirStruct) {
        if class.is_value_type {
            self.value_types.insert(class.name.clone());
            if let Some(declaration) = class.constructor_declaration {
                self.value_type_declarations.insert(declaration);
            }
        }
    }

    /// 登记依赖组导出的值类型声明身份。
    ///
    /// 跨组聚合只按 `constructor_declaration` 身份登记，不写短名集合：
    /// 短名可见性属于本组，导入声明不能凭名字影响本地 `Named` 判定。
    /// 数值 primitive（`usize` 等）在 core 声明、在 std 使用，正是这条路径。
    pub fn register_value_type_declaration(&mut self, class: &HirStruct) {
        if class.is_value_type {
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

    /// 判定赋值该类型时使用拷贝还是引用语义。
    pub fn validate_assignment(&self, ty: &HirType) -> AssignmentSemantics {
        if self.is_value(ty) { AssignmentSemantics::Copy } else { AssignmentSemantics::Reference }
    }

    /// 判定按该类型传参时使用拷贝还是引用语义。
    pub fn validate_parameter_passing(&self, ty: &HirType) -> ParameterSemantics {
        if self.is_value(ty) { ParameterSemantics::Copy } else { ParameterSemantics::Reference }
    }

    /// 判定按该类型返回时使用拷贝还是引用语义。
    pub fn validate_return(&self, ty: &HirType) -> ReturnSemantics {
        if self.is_value(ty) { ReturnSemantics::Copy } else { ReturnSemantics::Reference }
    }

    /// 判断给定类型是否为已注册的值类型或内建值语义类型（元组、定长数组）。
    pub fn is_value_type(&self, ty: &HirType) -> bool {
        self.is_value(ty)
    }

    /// 判断给定类型是否可被值类型按拷贝语义持有。
    ///
    /// 语言语义：值类型可含任意字段类型——class 实例、堆数组、`unite` 等引用类型
    /// 字段均合法；值类型的拷贝为浅拷贝，引用字段拷贝引用本身。
    /// 此前将堆数组、未注册命名类型等引用字段一律判非法属于过严误判
    /// （`copy discipline violation` 的 `ReferenceTypedField` 分支）。
    fn is_copy_compatible(&self, ty: &HirType) -> bool {
        // 停线清理：QualifiedPath 不得再被当作"安全 false"。到达这里说明源头未切断，panic 暴露上游问题。
        if matches!(ty, HirType::QualifiedPath { .. }) {
            panic!("QualifiedPath must not reach is_copy_compatible; frontend lowering must resolve it");
        }
        true
    }

    /// 声明期字段 copy 兼容：泛型形参在代入前不得按短名判引用。
    fn is_field_copy_compatible(&self, struct_def: &HirStruct, ty: &HirType) -> bool {
        match ty {
            HirType::Named(name) if struct_def.generics.iter().any(|generic| generic.name == *name) => true,
            HirType::Generic(generic) if struct_def.generics.iter().any(|item| item.name == generic.name) => true,
            _ => self.is_copy_compatible(ty),
        }
    }

    /// 在值类型结构体中查找首个引用类型字段，返回该字段引用以便定位违规。
    ///
    /// 当前语言语义下值类型可含任意字段类型（浅拷贝语义），本查询不产生违规字段；
    /// 保留该查询以稳定调用面，并作为未来布局级约束（如递归值类型）的落点。
    pub fn find_non_copy_field<'a>(&self, struct_def: &'a HirStruct) -> Option<&'a HirField> {
        struct_def.fields.iter().find(|field| !self.is_field_copy_compatible(struct_def, &field.ty))
    }
}
