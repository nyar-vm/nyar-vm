//! 重载域、候选签名与匹配类别。

use std::collections::BTreeSet;

use crate::compiler::types::{
    Identifier, NamePath,
    hir::{GenericBinderId, HirParam, HirWhereConstraint, ValkyrieType},
};

/// 候选所属的重载域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverloadDomain {
    /// 普通函数调用。
    Function,
    /// 构造调用。
    Constructor,
    /// 模式提取器调用。
    Extractor,
}

/// 候选的调用签名：形参类型列表与返回类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverloadSignature {
    /// 形参类型列表。
    pub params: Vec<ValkyrieType>,
    /// 返回类型。
    pub return_type: ValkyrieType,
}

/// 一个重载候选：稳定的声明身份、调用签名与匹配类别。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverloadCandidate {
    /// 候选的声明身份；未冻结时为 `None`。
    pub declaration: Option<nyar_types::ItemId>,
    /// 候选的实例身份；未冻结时为 `None`。
    pub instance: Option<nyar_types::ItemInstanceId>,
    /// 所属 unity 的声明身份。
    pub sum_owner: Option<nyar_types::ItemId>,
    /// 所属 unity 变体的声明身份。
    pub sum_variant: Option<nyar_types::ItemId>,
    /// 语言内建结构操作身份；只有语言内建候选持有，源码声明候选恒为 `None`。
    pub intrinsic: Option<crate::compiler::types::hir::BuiltinStructureOp>,
    /// 候选符号的限定路径。
    pub symbol: NamePath,
    /// 拥有该候选的类型名（方法候选）。
    pub owner: Option<Identifier>,
    /// 该候选所属 trait 的声明身份。
    pub trait_owner: Option<nyar_types::ItemId>,
    /// 候选所属的重载域。
    pub domain: OverloadDomain,
    /// 候选的调用签名。
    pub signature: OverloadSignature,
    /// 该候选的匹配类别。
    pub match_kind: OverloadMatchKind,
    /// 形参描述（含绑定方式与默认值）。
    pub param_specs: Vec<HirParam>,
    /// Formal type variables which must be bound from actual arguments.
    pub generic_binders: BTreeSet<GenericBinderId>,
    /// 声明级 where 合同；实参代入后必须取得对应 witness。
    pub where_constraints: Vec<HirWhereConstraint>,
    /// 来自 `imported_semantic_exports` 的顶层函数；无限定调用按导出名匹配，不按 consumer 模块路径。
    pub imported_dependency_export: bool,
}

impl OverloadCandidate {
    /// 以符号、域、形参列表、返回类型与匹配类别构造候选。
    pub fn new(
        symbol: NamePath,
        domain: OverloadDomain,
        params: Vec<ValkyrieType>,
        return_type: ValkyrieType,
        match_kind: OverloadMatchKind,
    ) -> Self {
        Self {
            declaration: None,
            instance: None,
            sum_owner: None,
            sum_variant: None,
            intrinsic: None,
            symbol,
            owner: None,
            trait_owner: None,
            domain,
            signature: OverloadSignature { params, return_type },
            match_kind,
            param_specs: Vec::new(),
            generic_binders: BTreeSet::new(),
            where_constraints: Vec::new(),
            imported_dependency_export: false,
        }
    }

    /// 附加形参描述。
    pub fn with_param_specs(mut self, param_specs: Vec<HirParam>) -> Self {
        self.param_specs = param_specs;
        self
    }

    /// 登记一个必须由实参绑定的形式类型变量。
    pub fn with_generic_binder(mut self, binder: GenericBinderId) -> Self {
        self.generic_binders.insert(binder);
        self
    }

    pub(crate) fn with_where_constraints(mut self, constraints: Vec<HirWhereConstraint>) -> Self {
        self.where_constraints = constraints;
        self
    }

    /// 构造带拥有者类型的方法候选。
    pub fn new_method(
        owner: Identifier,
        symbol: NamePath,
        domain: OverloadDomain,
        params: Vec<ValkyrieType>,
        return_type: ValkyrieType,
        match_kind: OverloadMatchKind,
    ) -> Self {
        Self {
            declaration: None,
            instance: None,
            sum_owner: None,
            sum_variant: None,
            intrinsic: None,
            symbol,
            owner: Some(owner),
            trait_owner: None,
            domain,
            signature: OverloadSignature { params, return_type },
            match_kind,
            param_specs: Vec::new(),
            generic_binders: BTreeSet::new(),
            where_constraints: Vec::new(),
            imported_dependency_export: false,
        }
    }
}

/// 候选参数匹配的类别，决定重载优先级。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverloadMatchKind {
    /// 名义类型完全一致。
    NominalExact,
    /// 名义子类型，距离越小越优先。
    NominalSubtype {
        /// 继承距离。
        distance: usize,
    },
    /// 通过具名 trait 满足。
    Trait,
    /// 通过匿名 row 形状满足。
    Row,
}

/// 重载解析失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverloadResolutionError {
    /// 没有任何候选匹配。
    NoMatch,
    /// 存在多个同优先级候选。
    Ambiguous {
        /// 并列候选的符号路径。
        candidates: Vec<NamePath>,
    },
}

/// 按匹配类别的既定优先级选出唯一候选；并列时报歧义。
pub fn resolve_overload(candidates: &[OverloadCandidate]) -> Result<OverloadCandidate, OverloadResolutionError> {
    let best_rank = candidates.iter().map(|candidate| rank(&candidate.match_kind)).min().ok_or(OverloadResolutionError::NoMatch)?;
    let mut tied = Vec::new();
    for candidate in candidates.iter().filter(|candidate| rank(&candidate.match_kind) == best_rank) {
        if !tied.contains(&candidate) {
            tied.push(candidate);
        }
    }
    match tied.as_slice() {
        [selected] => Ok((*selected).clone()),
        [] => Err(OverloadResolutionError::NoMatch),
        ambiguous => {
            // 源码声明携带声明身份与冻结的 intrinsic 合同，语言 builtin 注册表只是
            // 没有对应声明时的语言级种子；同优先级时声明优先，而不是让两个权威并列。
            let declared = ambiguous.iter().filter(|candidate| candidate.declaration.is_some()).collect::<Vec<_>>();
            if let [selected] = declared.as_slice() {
                return Ok((**selected).clone());
            }
            Err(OverloadResolutionError::Ambiguous { candidates: ambiguous.iter().map(|candidate| candidate.symbol.clone()).collect() })
        }
    }
}

pub(crate) fn rank(match_kind: &OverloadMatchKind) -> (u8, usize) {
    match match_kind {
        OverloadMatchKind::NominalExact => (0, 0),
        OverloadMatchKind::NominalSubtype { distance } => (1, *distance),
        OverloadMatchKind::Trait => (2, 0),
        OverloadMatchKind::Row => (3, 0),
    }
}
