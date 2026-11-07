//! 重载域、候选签名与匹配类别。

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::types::{
    Identifier, NamePath,
    hir::{GenericBinderId, HirKind, HirParam, HirWhereConstraint, ValkyrieType},
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
    /// 候选符号的限定路径。
    pub symbol: NamePath,
    /// 拥有该候选的类型名（方法候选）。这是**可调用身份**的短名，不承担接收者
    /// 参与判定：泛型头部 impl（如 `imply<T> ...`）没有可用的短名时为 `None`。
    pub owner: Option<Identifier>,
    /// 该候选所属 impl 的完整头部目标类型（含泛型实参，如 `Array<T>`）。它才是
    /// **impl 身份**：接收者形态（具体 / 泛型）由它与实参的结构统一决定。trait 槽
    /// 候选没有 impl 身份，因此为 `None`，只由上下文 bound 决定是否参与。
    pub impl_target: Option<ValkyrieType>,
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
    /// 每个 binder 声明的形参种类；只来自声明处 `GenericType`，用于显式泛型实参
    /// 诊断如实列出期望种类。集合里出现但没有登记种类的 binder（例如从 owner 类型
    /// 实参里收集到的 binder）不在诊断里虚构种类。
    pub generic_binder_kinds: BTreeMap<GenericBinderId, HirKind>,
    /// 声明级 where 合同；实参代入后必须取得对应 witness。
    pub where_constraints: Vec<HirWhereConstraint>,
    /// 来自 `imported_semantic_exports` 的顶层函数；无限定调用按导出名匹配，不按 consumer 模块路径。
    pub imported_dependency_export: bool,
    /// 该构造候选是否来自带 `[constructor]` 的 `unite` 变体，决定括号调用入口是否可见。
    pub is_auto_constructor: bool,
    /// 调用点已结算的泛型 binder 代入；只有经 where 校验通过的候选才有值，
    /// 未经过该路径的候选保持空表，由单态化侧按既有 fail-closed 处理。
    pub associated_binder_solutions: BTreeMap<GenericBinderId, ValkyrieType>,
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
            symbol,
            owner: None,
            impl_target: None,
            trait_owner: None,
            domain,
            signature: OverloadSignature { params, return_type },
            match_kind,
            param_specs: Vec::new(),
            generic_binders: BTreeSet::new(),
            generic_binder_kinds: BTreeMap::new(),
            where_constraints: Vec::new(),
            imported_dependency_export: false,
            is_auto_constructor: false,
            associated_binder_solutions: BTreeMap::new(),
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

    /// 登记声明处的泛型形参：binder 身份与形参种类同时来自同一份 `GenericType`。
    pub fn with_generic_parameter(mut self, generic: &crate::compiler::types::hir::GenericType) -> Self {
        if let Some(binder) = generic.binder {
            self.generic_binders.insert(binder);
            self.generic_binder_kinds.insert(binder, generic.kind.clone());
        }
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
            symbol,
            owner: Some(owner),
            impl_target: None,
            trait_owner: None,
            domain,
            signature: OverloadSignature { params, return_type },
            match_kind,
            param_specs: Vec::new(),
            generic_binders: BTreeSet::new(),
            generic_binder_kinds: BTreeMap::new(),
            where_constraints: Vec::new(),
            imported_dependency_export: false,
            is_auto_constructor: false,
            associated_binder_solutions: BTreeMap::new(),
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

/// 数字字面量对候选数值形参的绑定代价。
///
/// 只在同匹配类别候选并列时作为裁决输入，不改变单候选的绑定结果：`fits` 表示
/// 实参字面量的值能否落入形参类型，`coercions` 表示相对字面量自然类型需要改绑的
/// 宽度层数。两项仍不能唯一裁决时继续走声明身份 / inherent 规则，最后如实报歧义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NumericLiteralBinding {
    /// 实参字面量的值是否都能落入候选的数值形参类型。
    pub fits: bool,
    /// 相对字面量自然类型需要改绑的宽度层数（越少越接近默认推导）。
    pub coercions: usize,
}

/// 按匹配类别的既定优先级选出唯一候选；并列时报歧义。
pub fn resolve_overload(candidates: &[OverloadCandidate]) -> Result<OverloadCandidate, OverloadResolutionError> {
    resolve_overload_inner(candidates, None, None)
}

/// 与 [`resolve_overload`] 相同，但当全部并列候选都带数字字面量绑定代价时，
/// 先按「值可落入形参类型 → 改绑层数最少」裁决；无法唯一裁决时回落既定规则。
pub(crate) fn resolve_overload_with_numeric_literals(
    candidates: &[OverloadCandidate],
    bindings: &[Option<NumericLiteralBinding>],
    contextual_type: Option<&ValkyrieType>,
) -> Result<OverloadCandidate, OverloadResolutionError> {
    resolve_overload_inner(candidates, Some(bindings), contextual_type)
}

fn resolve_overload_inner(
    candidates: &[OverloadCandidate],
    bindings: Option<&[Option<NumericLiteralBinding>]>,
    contextual_type: Option<&ValkyrieType>,
) -> Result<OverloadCandidate, OverloadResolutionError> {
    let best_rank = candidates.iter().map(|candidate| rank(&candidate.match_kind)).min().ok_or(OverloadResolutionError::NoMatch)?;
    let mut tied = Vec::new();
    let mut tied_bindings = Vec::new();
    for (index, candidate) in candidates.iter().enumerate().filter(|(_, candidate)| rank(&candidate.match_kind) == best_rank) {
        if !tied.contains(&candidate) {
            tied.push(candidate);
            tied_bindings.push(bindings.and_then(|bindings| bindings.get(index).copied().flatten()));
        }
    }
    match tied.as_slice() {
        [selected] => Ok((*selected).clone()),
        [] => Err(OverloadResolutionError::NoMatch),
        ambiguous => {
            // 调用点期望类型是权威上下文：并列候选中恰有一个返回该类型时直接采用，
            // 这比字面量默认宽度更可靠。
            if let Some(contextual_type) = contextual_type
                && let Some(selected) = select_by_return_context(ambiguous, contextual_type)
            {
                return Ok(selected.clone());
            }
            // 全部并列候选都能量化字面量绑定代价时才裁决；只要有一个无法比较，
            // 就不得用不完整信息猜测，直接回落既定规则。
            if tied_bindings.iter().all(Option::is_some)
                && let Some(selected) = select_by_numeric_literal_binding(ambiguous, &tied_bindings)
            {
                return Ok(selected.clone());
            }
            // 同优先级时携带声明身份的候选优先，而不是让两个权威并列。
            let declared = ambiguous.iter().filter(|candidate| candidate.declaration.is_some()).collect::<Vec<_>>();
            if let [selected] = declared.as_slice() {
                return Ok((**selected).clone());
            }
            // 身份俱在仍并列时，inherent impl 成员严格优先于 trait impl 成员：
            // `imply u64` 与 `imply u64: Equal` 的同形 `==` 并存是合法的（u64 既要有
            // 原生比较也要有 trait 证据），按方法解析的既定语义由 inherent 胜出，
            // 不得让同形对撞退化成「无一候选」。
            let inherent = ambiguous.iter().filter(|candidate| candidate.trait_owner.is_none()).collect::<Vec<_>>();
            if let [selected] = inherent.as_slice() {
                return Ok((**selected).clone());
            }
            Err(OverloadResolutionError::Ambiguous { candidates: ambiguous.iter().map(|candidate| candidate.symbol.clone()).collect() })
        }
    }
}

/// 并列候选中恰有一个返回调用点期望类型时选中它；否则返回 `None`，不得猜测。
fn select_by_return_context<'a>(
    ambiguous: &[&'a OverloadCandidate],
    contextual_type: &ValkyrieType,
) -> Option<&'a OverloadCandidate> {
    let matching = ambiguous
        .iter()
        .filter(|candidate| candidate.signature.return_type == *contextual_type)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [selected] => Some(**selected),
        _ => None,
    }
}

/// 在并列候选中按数字字面量绑定代价选出唯一者：先要求字面量值能落入形参类型，
/// 再取改绑层数最少者；仍不唯一则返回 `None`，交由上层继续按身份规则裁决。
fn select_by_numeric_literal_binding<'a>(
    ambiguous: &[&'a OverloadCandidate],
    bindings: &[Option<NumericLiteralBinding>],
) -> Option<&'a OverloadCandidate> {
    let fitting = ambiguous
        .iter()
        .zip(bindings)
        .filter(|(_, binding)| matches!(binding, Some(binding) if binding.fits))
        .collect::<Vec<_>>();
    let min_coercions = fitting.iter().filter_map(|(_, binding)| binding.as_ref().map(|binding| binding.coercions)).min()?;
    let narrowed = fitting
        .into_iter()
        .filter(|(_, binding)| binding.as_ref().is_some_and(|binding| binding.coercions == min_coercions))
        .collect::<Vec<_>>();
    match narrowed.as_slice() {
        [(selected, _)] => Some(**selected),
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn operator_candidate(owner: &str, ty: ValkyrieType) -> OverloadCandidate {
        OverloadCandidate::new(
            NamePath::new(vec![Identifier::new("infix"), Identifier::new(owner)]),
            OverloadDomain::Function,
            vec![ty.clone(), ty.clone()],
            ty,
            OverloadMatchKind::NominalExact,
        )
    }

    fn selected_owner(resolved: Result<OverloadCandidate, OverloadResolutionError>) -> String {
        resolved
            .expect("数字字面量并列候选应能唯一裁决")
            .symbol
            .parts()
            .last()
            .expect("候选符号非空")
            .as_str()
            .to_owned()
    }

    #[test]
    fn numeric_literal_binding_prefers_default_width() {
        let candidates = vec![
            operator_candidate("i32", ValkyrieType::Integer32 { signed: true }),
            operator_candidate("i64", ValkyrieType::Integer64 { signed: true }),
            operator_candidate("u32", ValkyrieType::Integer32 { signed: false }),
        ];
        let bindings = vec![
            Some(NumericLiteralBinding { fits: true, coercions: 0 }),
            Some(NumericLiteralBinding { fits: true, coercions: 2 }),
            Some(NumericLiteralBinding { fits: true, coercions: 2 }),
        ];
        assert_eq!(selected_owner(resolve_overload_with_numeric_literals(&candidates, &bindings, None)), "i32");
    }

    #[test]
    fn return_context_overrides_literal_default_width() {
        let candidates = vec![
            operator_candidate("i32", ValkyrieType::Integer32 { signed: true }),
            operator_candidate("i64", ValkyrieType::Integer64 { signed: true }),
            operator_candidate("u32", ValkyrieType::Integer32 { signed: false }),
        ];
        let bindings = vec![
            Some(NumericLiteralBinding { fits: true, coercions: 0 }),
            Some(NumericLiteralBinding { fits: true, coercions: 2 }),
            Some(NumericLiteralBinding { fits: true, coercions: 2 }),
        ];
        let contextual = ValkyrieType::Integer64 { signed: true };
        assert_eq!(
            selected_owner(resolve_overload_with_numeric_literals(&candidates, &bindings, Some(&contextual))),
            "i64"
        );
    }

    #[test]
    fn numeric_literal_binding_prefers_representable_wider_candidate() {
        let candidates = vec![
            operator_candidate("i32", ValkyrieType::Integer32 { signed: true }),
            operator_candidate("i64", ValkyrieType::Integer64 { signed: true }),
        ];
        let bindings = vec![
            Some(NumericLiteralBinding { fits: false, coercions: 1 }),
            Some(NumericLiteralBinding { fits: true, coercions: 1 }),
        ];
        assert_eq!(selected_owner(resolve_overload_with_numeric_literals(&candidates, &bindings, None)), "i64");
    }

    #[test]
    fn numeric_literal_binding_tie_stays_ambiguous() {
        let candidates = vec![
            operator_candidate("i32", ValkyrieType::Integer32 { signed: true }),
            operator_candidate("i64", ValkyrieType::Integer64 { signed: true }),
        ];
        let bindings =
            vec![Some(NumericLiteralBinding { fits: true, coercions: 1 }), Some(NumericLiteralBinding { fits: true, coercions: 1 })];
        assert!(matches!(
            resolve_overload_with_numeric_literals(&candidates, &bindings, None),
            Err(OverloadResolutionError::Ambiguous { .. })
        ));
    }
}
