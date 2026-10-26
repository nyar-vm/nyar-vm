//! 符号匹配：symbol_matches_unqualified_free_call / candidate_*_matches 等。

use crate::compiler::types::{Identifier, NamePath};

use super::diagnostics::{OverloadCandidate, OverloadDomain};

/// 无限定自由函数调用：按当前模块结构化路径硬匹配完整 identity，禁止末段字符串兜底。
pub(crate) fn symbol_matches_unqualified_free_call(symbol: &NamePath, callee_name: &Identifier, enclosing_module: &NamePath) -> bool {
    let mut expected = enclosing_module.parts().to_vec();
    expected.push(callee_name.clone());
    symbol.parts() == expected.as_slice()
}

/// 依赖语义导出中的顶层函数：consumer 内无限定调用按导出名匹配。
pub(crate) fn symbol_matches_imported_dependency_export(candidate: &OverloadCandidate, callee_name: &Identifier) -> bool {
    if !candidate.imported_dependency_export {
        return false;
    }
    candidate.symbol.parts().last() == Some(callee_name)
}

/// 语言内建候选（如数组下标）不属于任何模块，symbol 是注册表内的单段结构名。
pub(crate) fn symbol_matches_language_builtin(candidate: &OverloadCandidate, callee_name: &Identifier) -> bool {
    candidate.declaration.is_none() && candidate.intrinsic.is_some() && matches!(candidate.symbol.parts(), [name] if name == callee_name)
}

/// 自有/静态成员候选：符号为单段成员名，由 `owner` / `trait_owner` 携带归属。
pub(crate) fn candidate_owned_method_name(candidate: &OverloadCandidate) -> Option<&Identifier> {
    match candidate.symbol.parts() {
        [name] => Some(name),
        _ => None,
    }
}

pub(crate) fn candidate_owned_method_matches(candidate: &OverloadCandidate, method_name: &Identifier) -> bool {
    candidate_owned_method_name(candidate) == Some(method_name)
}

/// trait 默认/声明方法：符号为 `[Trait, method]` 或单段 `method`（`trait_owner` 已冻结）。
pub(crate) fn candidate_trait_method_matches(candidate: &OverloadCandidate, method_name: &Identifier) -> bool {
    match candidate.symbol.parts() {
        [name] => name == method_name,
        [_, name] if candidate.trait_owner.is_some() => name == method_name,
        _ => false,
    }
}

pub(crate) fn candidate_instance_method_matches(candidate: &OverloadCandidate, method_name: &Identifier) -> bool {
    if candidate.trait_owner.is_some() {
        candidate_trait_method_matches(candidate, method_name)
    }
    else {
        candidate_owned_method_matches(candidate, method_name)
    }
}

/// 变体构造候选：`owner` 为枚举短名，符号为单段变体名。
pub(crate) fn candidate_variant_constructor_matches(candidate: &OverloadCandidate, variant_name: &Identifier) -> bool {
    candidate.domain == OverloadDomain::Constructor
        && candidate.owner.is_some()
        && matches!(candidate.symbol.parts(), [name] if name == variant_name)
}

/// 聚合构造候选：限定路径按完整 identity 对齐，未限定短名按已冻结的声明归属对齐。
pub(crate) fn aggregate_constructor_matches_name(candidate: &OverloadCandidate, path: &NamePath) -> bool {
    if candidate.domain != OverloadDomain::Constructor {
        return false;
    }
    let path_parts = path.parts();
    if let Some(expected_owner) = candidate.owner.as_ref() {
        let Some(variant_name) = candidate.symbol.parts().first()
        else {
            return false;
        };
        match path_parts {
            [owner, variant] => owner == expected_owner && variant == variant_name,
            [variant] => variant == variant_name,
            _ => false,
        }
    }
    else {
        candidate.symbol.parts() == path_parts || (path_parts.len() == 1 && candidate.symbol.parts().last() == path_parts.last())
    }
}

/// 模式 extractors：`Owner.member` 与候选 `owner` + 单段符号精确对齐。
pub(crate) fn candidate_owned_member_path_matches(candidate: &OverloadCandidate, owner: &Identifier, member: &Identifier) -> bool {
    candidate.owner.as_ref() == Some(owner) && matches!(candidate.symbol.parts(), [name] if name == member)
}

pub(crate) fn candidate_extractor_path_matches(canonical_callee: &NamePath, candidate: &OverloadCandidate) -> bool {
    match canonical_callee.parts() {
        [owner, member] => candidate_owned_member_path_matches(candidate, owner, member),
        _ => false,
    }
}
