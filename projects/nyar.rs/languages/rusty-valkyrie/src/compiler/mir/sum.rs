//! Semantic MIR 的 sum 声明合同；不持有物理 tag 宽度或目标布局。

use std::collections::BTreeMap;

use crate::compiler::types::hir::{GenericType, ValkyrieType};
use nyar_types::{AggregateLayoutPlan, Identifier, NyarType};

/// 保留声明的泛型参数与 variant 类型；实例化必须先于物理表示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirSumDeclaration {
    /// Semantic MIR 冻结的名义声明身份。
    pub nominal: nyar_types::NominalInstanceId,
    /// HIR 冻结的 sum 声明身份。
    pub declaration: Option<nyar_types::ItemId>,
    /// 声明 owner；身份迁移不得由布局反推。
    pub name: String,
    /// 语言声明是否允许 variant 子类型。
    pub is_unite: bool,
    /// 值语义事实：enum/unite 声明决定，不是目标推断。
    pub is_value_type: bool,
    /// 声明 binder、kind 与 bounds。
    pub generics: Vec<GenericType>,
    /// 声明顺序中的 variant。
    pub variants: Vec<MirSumVariant>,
}
/// variant 的 payload 与 GADT 结果类型属于声明事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirSumVariant {
    /// Semantic MIR 冻结的 variant 身份。
    pub id: nyar_types::VariantId,
    /// HIR 冻结的 variant 构造声明身份。
    pub declaration: Option<nyar_types::ItemId>,
    /// 声明名称。
    pub name: String,
    /// 源码判别值。
    pub tag: u32,
    /// 原始字段身份与语义类型。
    pub fields: Vec<super::MirField>,
    /// 显式 GADT 结果约束。
    pub result_type: Option<ValkyrieType>,
}

impl MirSumVariant {
    /// 多字段 payload 是名义变体记录（身份 = 变体 `declaration`），不保留位置元组语义。
    ///
    /// 记录类型本身不带实参；泛型枚举的实参由 [`MirSumDeclaration::instantiate_payload`]
    /// 代入，同一枚举的不同变体因此各有独立记录身份。
    pub fn payload_type(&self) -> Option<ValkyrieType> {
        match self.fields.as_slice() {
            [] => None,
            [field] => Some(field.ty.clone()),
            _ => self.declaration.map(|declaration| ValkyrieType::Nominal {
                declaration,
                arguments: Vec::new(),
                primitive_identity: None,
            }),
        }
    }
}

impl MirSumDeclaration {
    /// 按 HIR 冻结的 `ItemId` 查找唯一 sum 声明。禁止短名/限定名字符串回查。
    pub fn find_by_declaration<'a>(declarations: &'a [Self], declaration: nyar_types::ItemId) -> Option<&'a Self> {
        let matches: Vec<&Self> = declarations.iter().filter(|item| item.declaration == Some(declaration)).collect();
        if matches.len() == 1 { Some(matches[0]) } else { None }
    }

    /// 按变体记录自身冻结的 `ItemId` 查找唯一 `(sum, variant)`。
    ///
    /// 记录身份只来自 `variant.declaration`；同一枚举的不同变体各自命中不同记录，
    /// 不会共享一个身份。禁止按变体短名回查。
    pub fn find_record_variant<'a>(declarations: &'a [Self], declaration: nyar_types::ItemId) -> Option<(&'a Self, &'a MirSumVariant)> {
        let matches: Vec<(&Self, &MirSumVariant)> =
            declarations.iter().flat_map(|sum| sum.variants.iter().filter(move |variant| variant.declaration == Some(declaration)).map(move |variant| (sum, variant))).collect();
        if matches.len() == 1 { Some(matches[0]) } else { None }
    }

    /// 完成声明结果或 variant refinement 的同一泛型代入。
    pub fn instantiate_result(&self, variant: &MirSumVariant, arguments: &[ValkyrieType]) -> Option<ValkyrieType> {
        if self.generics.len() != arguments.len() {
            return None;
        }
        if let Some(result) = &variant.result_type {
            let substitutions = self
                .generics
                .iter()
                .zip(arguments)
                .map(|(generic, argument)| Some((generic.binder?, argument.clone())))
                .collect::<Option<BTreeMap<_, _>>>()?;
            return Some(crate::compiler::hir::overload::substitute_type_vars(result, &substitutions));
        }
        let declaration = self.declaration?;
        // sum（枚举）不是 `[primitive]` 声明，没有自举身份。
        Some(ValkyrieType::Nominal { declaration, arguments: arguments.to_vec(), primitive_identity: None })
    }
    /// 只按声明 binder 代入，不依据 variant 拼写选择实参位置。
    ///
    /// 多字段变体实例化为「变体记录 × 枚举实参」：记录身份的实参就是枚举实参本身，
    /// 不是字段类型的位置元组。单字段变体仍沿用字段类型代入后的结果。
    pub fn instantiate_payload(&self, variant: &MirSumVariant, arguments: &[ValkyrieType]) -> Option<Option<ValkyrieType>> {
        if self.generics.len() != arguments.len() {
            return None;
        }
        // 无字段变体没有 payload，和「代入失败」必须区分：前者是合法的 `None` 载荷。
        if variant.fields.is_empty() {
            return Some(None);
        }
        let substitutions = self
            .generics
            .iter()
            .zip(arguments)
            .map(|(generic, argument)| Some((generic.binder?, argument.clone())))
            .collect::<Option<BTreeMap<_, _>>>()?;
        if variant.fields.len() == 1 {
            return Some(Some(crate::compiler::hir::overload::substitute_type_vars(&variant.fields[0].ty, &substitutions)));
        }
        let declaration = variant.declaration?;
        Some(Some(ValkyrieType::Nominal { declaration, arguments: arguments.to_vec(), primitive_identity: None }))
    }

    /// 记录字段类型的实例化结果：字段仍按声明 binder 逐个代入。
    pub fn instantiate_field_types(&self, variant: &MirSumVariant, arguments: &[ValkyrieType]) -> Option<Vec<ValkyrieType>> {
        if self.generics.len() != arguments.len() {
            return None;
        }
        let substitutions = self
            .generics
            .iter()
            .zip(arguments)
            .map(|(generic, argument)| Some((generic.binder?, argument.clone())))
            .collect::<Option<BTreeMap<_, _>>>()?;
        Some(variant.fields.iter().map(|field| crate::compiler::hir::overload::substitute_type_vars(&field.ty, &substitutions)).collect())
    }
}

/// 变体记录 payload 的唯一布局口径：声明身份 → 已登记 `AggregateLayout` → 限定名。
///
/// Canonical 的声明级与实例级 sum 布局共用这条链路，避免两套拼写口径。
/// 未登记（含泛型实例尚未生成记录布局）一律返回 `None`，由调用方失败关闭。
pub(crate) fn record_payload_layout(record: &ValkyrieType, layouts: &AggregateLayoutPlan) -> Option<NyarType> {
    let ValkyrieType::Nominal { declaration, .. } = record
    else {
        return None;
    };
    let key = ValkyrieType::Nominal { declaration: *declaration, arguments: Vec::new(), primitive_identity: None };
    let layout_id = crate::compiler::mir::layout_id_for_type(&key, layouts)?;
    let layout = layouts.layouts.iter().find(|layout| layout.id == layout_id)?;
    let qualified = if layout.namespace.is_empty() {
        layout.name.clone()
    }
    else {
        format!("{}::{}", layout.namespace.replace('.', "::"), layout.name)
    };
    Some(NyarType::Named(Identifier::new(&qualified)))
}
