//! `[when(expression)]`：目标/环境条件（与 `gate` 独立；多个 `when` item 不在此层 AND/OR）。
//!
//! `target.vendor` / `target.specification` 由 target registry 投影到四元组，不引入独立 platform 轴：
//! - `target.vendor == "browser"` → Browser 规格
//! - `target.vendor == "wechat"` → Miniprogram 规格
//! - `target.specification == "wasip3"` / `"wasi"` → ABI / Wasi 规格
//!
//! 不替代 Compiler 前端的条件 lowering（HIR 仍待对齐）。

use crate::compiler::syntax::oak::ast::{Attribute, TermExpression};
use oak_valkyrie::lexer::token_type::ValkyrieTokenType;

use nyar::{CanonicalAbi, CanonicalSpecification, CanonicalTarget, CanonicalVendor};

use super::ast_attributes::term_string_literal;
use super::types::WhenPredicate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WhenConditionKey {
    TargetSpecification,
    TargetVendor,
}

impl WhenPredicate {
    pub(crate) fn from_when_attribute(attribute: &Attribute) -> Option<Self> {
        if attribute.name.name != "when" {
            return None;
        }
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

    fn condition_key(&self) -> Option<WhenConditionKey> {
        match self.key.as_str() {
            "target_specification" | "specification" => Some(WhenConditionKey::TargetSpecification),
            "target_vendor" | "vendor" => Some(WhenConditionKey::TargetVendor),
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

fn condition_atom_matches(target: CanonicalTarget, key: WhenConditionKey, value: &str) -> bool {
    match key {
        WhenConditionKey::TargetSpecification => target_matches_specification_token(target, value),
        WhenConditionKey::TargetVendor => vendor_matches_token(target, value),
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

/// 单个 `when` attribute item 是否匹配当前 target。
pub(crate) fn when_item_matches_target(predicate: &WhenPredicate, target: CanonicalTarget) -> bool {
    predicate
        .condition_key()
        .map(|key| condition_atom_matches(target, key, &predicate.value))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar::CanonicalTarget;

    #[test]
    fn when_vendor_browser_and_wechat_map_to_specifications() {
        let browser = CanonicalTarget::browser();
        let mp: CanonicalTarget = "wasm32-unknown-miniprogram-wasm".parse().unwrap();
        let browser_pred = WhenPredicate { key: "vendor".into(), value: "browser".into() };
        let wechat_pred = WhenPredicate { key: "vendor".into(), value: "wechat".into() };
        assert!(when_item_matches_target(&browser_pred, browser));
        assert!(!when_item_matches_target(&wechat_pred, browser));
        assert!(when_item_matches_target(&wechat_pred, mp));
        assert!(!when_item_matches_target(&browser_pred, mp));
    }
}
