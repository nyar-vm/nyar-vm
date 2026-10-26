//! Planner 侧对 `[cfg(...)]` / `[when(...)]` 的**最小**求值：文件级筛选与 host provider 选择；不替代前端 cfg lowering。
//!
//! `cfg(key = "value")` 与 `when(target.specification == "wasip3")` 均归一为 `CfgPredicate`；
//! 正式 registry 规格名见 `target_matches_specification_token`。

use crate::compiler::syntax::oak::ast::{Attribute, TermExpression};
use oak_valkyrie::lexer::token_type::ValkyrieTokenType;

use nyar::{CanonicalAbi, CanonicalSpecification, CanonicalTarget, CanonicalVendor};

use super::ast_attributes::term_string_literal;
use super::types::{CfgPredicate, PlannedHostProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CfgKey {
    TargetSpecification,
    TargetVendor,
}

impl CfgPredicate {
    pub(crate) fn from_compile_condition_attribute(attribute: &Attribute) -> Option<Self> {
        match attribute.name.name.as_str() {
            "cfg" => Self::from_cfg_attribute(attribute),
            "when" => Self::from_when_attribute(attribute),
            _ => None,
        }
    }

    fn from_cfg_attribute(attribute: &Attribute) -> Option<Self> {
        let argument = attribute.args.first()?;
        let key = argument.key.as_ref()?.name.as_str();
        let value = term_string_literal(&argument.value)?;
        if value.is_empty() {
            return None;
        }
        Some(Self { key: key.to_string(), value })
    }

    fn from_when_attribute(attribute: &Attribute) -> Option<Self> {
        let argument = attribute.args.first()?;
        if let Some(key) = &argument.key {
            let value = term_string_literal(&argument.value)?;
            if value.is_empty() {
                return None;
            }
            return Some(Self { key: key.name.clone(), value });
        }
        let (key, value) = parse_target_equality_expression(&argument.value)?;
        if value.is_empty() {
            return None;
        }
        Some(Self { key, value })
    }

    fn cfg_key(&self) -> Option<CfgKey> {
        match self.key.as_str() {
            "target_specification" | "specification" => Some(CfgKey::TargetSpecification),
            "target_vendor" | "vendor" => Some(CfgKey::TargetVendor),
            _ => None,
        }
    }
}

fn target_matches_specification_token(target: CanonicalTarget, token: &str) -> bool {
    match token.to_ascii_lowercase().as_str() {
        "browser-wasm" | "browser" | "web" => {
            target.specification == CanonicalSpecification::Browser && target.abi == Some(CanonicalAbi::WebAssembly)
        }
        "wechat-wasm" | "miniprogram" | "mini-program" | "wechat" => target.specification == CanonicalSpecification::Miniprogram,
        "wasip3" | "wasi-p3" => target.abi == Some(CanonicalAbi::WasiP3),
        "wasi" | "wasip2" | "wasi-p2" => {
            target.specification == CanonicalSpecification::Wasi && target.abi != Some(CanonicalAbi::WasiP3)
        }
        _ => false,
    }
}

fn vendor_matches_token(target: CanonicalTarget, token: &str) -> bool {
    match token.to_ascii_lowercase().as_str() {
        "browser" => target.specification == CanonicalSpecification::Browser,
        "wechat" => target.specification == CanonicalSpecification::Miniprogram,
        "node" => matches!(target.vendor, CanonicalVendor::Node | CanonicalVendor::Deno | CanonicalVendor::Bun),
        "unknown" => target.vendor == CanonicalVendor::Unknown,
        _ => false,
    }
}

fn cfg_atom_matches(target: CanonicalTarget, key: CfgKey, value: &str) -> bool {
    match key {
        CfgKey::TargetSpecification => target_matches_specification_token(target, value),
        CfgKey::TargetVendor => vendor_matches_token(target, value),
    }
}

fn parse_target_equality_expression(expr: &TermExpression) -> Option<(String, String)> {
    let TermExpression::Binary(binary) = expr else { return None };
    if binary.operator != ValkyrieTokenType::EqEq {
        return None;
    }
    let field = target_condition_field(&binary.lhs)?;
    let value = term_string_literal(&binary.rhs)?;
    Some((field, value))
}

fn target_condition_field(expr: &TermExpression) -> Option<String> {
    match expr {
        TermExpression::DotCall { receiver, field, .. } if is_target_receiver(receiver) => Some(field.name.clone()),
        _ => None,
    }
}

fn is_target_receiver(expr: &TermExpression) -> bool {
    matches!(expr, TermExpression::NamePath(path) if path.parts.len() == 1 && path.parts[0].name == "target")
}

fn predicate_matches_target(predicate: &CfgPredicate, target: CanonicalTarget) -> bool {
    predicate
        .cfg_key()
        .map(|key| cfg_atom_matches(target, key, &predicate.value))
        .unwrap_or(false)
}

/// 声明或文件级 cfg 谓词是否全部满足当前 target（空列表恒真）。
pub(crate) fn cfg_predicates_match_target(predicates: &[CfgPredicate], target: CanonicalTarget) -> bool {
    predicates.iter().all(|predicate| predicate_matches_target(predicate, target))
}

/// provider 上累积的 cfg 条件是否全部满足当前 target（无 cfg 则恒真）。
pub(crate) fn provider_cfg_matches_target(provider: &PlannedHostProvider, target: CanonicalTarget) -> bool {
    cfg_predicates_match_target(&provider.cfg_predicates, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar::CanonicalTarget;

    #[test]
    fn cfg_vendor_browser_and_wechat_map_to_specifications() {
        let browser = CanonicalTarget::wasm();
        let mp: CanonicalTarget = "wasm32-unknown-miniprogram-wasm".parse().unwrap();
        let browser_pred = CfgPredicate { key: "target_vendor".into(), value: "browser".into() };
        let wechat_pred = CfgPredicate { key: "target_vendor".into(), value: "wechat".into() };
        assert!(predicate_matches_target(&browser_pred, browser));
        assert!(!predicate_matches_target(&wechat_pred, browser));
        assert!(predicate_matches_target(&wechat_pred, mp));
        assert!(!predicate_matches_target(&browser_pred, mp));
    }
}
