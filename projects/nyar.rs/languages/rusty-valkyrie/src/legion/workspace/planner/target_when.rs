//! `[when(expression)]`：目标/环境条件（与 `gate` 独立；多个 `when` item 不在此层 AND/OR）。
//!
//! `target.vendor` / `target.specification` 由 target registry 投影到四元组，不引入独立 platform 轴：
//! - `target.specification == "browser"` → Browser 规格（DOM / Web API）
//! - `target.vendor == "wechat"` → Wechat vendor（小程序 / 小游戏 runtime）
//! - `target.vendor == "node"` / `"deno"` / `"bun"` → 对应 JS runtime vendor
//! - 两者**不**互相兼容：vendor="wechat" 不会匹配 specification="browser"，反之亦然
//! - `target.specification == "wasip3"` / `"wasi"` → ABI / Wasi 规格
//!
//! canonical triple 拼写：wechat → `wasm32-wechat-unknown-wasm`（vendor=Wechat，specification=Unknown）。

use crate::compiler::syntax::oak::ast::{Attribute, TermExpression};
use oak_valkyrie::lexer::token_type::ValkyrieTokenType;

use nyar::{CanonicalAbi, CanonicalSpecification, CanonicalTarget, CanonicalVendor};

use super::{ast_attributes::term_string_literal, types::WhenPredicate};

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

/// `target.specification == "..."` 条件匹配。
/// 只匹配 CanonicalSpecification 字段，不再跨轴映射 vendor。
fn target_matches_specification_token(target: CanonicalTarget, token: &str) -> bool {
    match token.to_ascii_lowercase().as_str() {
        "browser-wasm" | "browser" | "web" => {
            target.specification == CanonicalSpecification::Browser && target.abi == Some(CanonicalAbi::WebAssembly)
        }
        "wasip3" | "wasi-p3" => target.abi == Some(CanonicalAbi::WasiP3),
        "wasi" | "wasip2" | "wasi-p2" => target.specification == CanonicalSpecification::Wasi && target.abi != Some(CanonicalAbi::WasiP3),
        _ => false,
    }
}

/// `target.vendor == "..."` 条件匹配。
/// 只匹配 CanonicalVendor 字段，不再把 vendor 值映射到 specification（那是历史错误）。
fn vendor_matches_token(target: CanonicalTarget, token: &str) -> bool {
    match token.to_ascii_lowercase().as_str() {
        "wechat" => target.vendor == CanonicalVendor::Wechat,
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
    let TermExpression::Binary(binary) = expr
    else {
        return None;
    };
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
    predicate.condition_key().map(|key| condition_atom_matches(target, key, &predicate.value)).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar::CanonicalTarget;

    #[test]
    fn specification_browser_matches_only_browser_spec() {
        let browser = CanonicalTarget::browser();
        let wechat = CanonicalTarget::wechat();
        let spec_browser = WhenPredicate { key: "specification".into(), value: "browser".into() };
        assert!(when_item_matches_target(&spec_browser, browser));
        assert!(!when_item_matches_target(&spec_browser, wechat));
    }

    #[test]
    fn vendor_wechat_matches_only_wechat_vendor() {
        let browser = CanonicalTarget::browser();
        let wechat = CanonicalTarget::wechat();
        let vendor_wechat = WhenPredicate { key: "vendor".into(), value: "wechat".into() };
        assert!(when_item_matches_target(&vendor_wechat, wechat));
        assert!(!when_item_matches_target(&vendor_wechat, browser));
    }

    #[test]
    fn vendor_node_matches_js_runtimes() {
        let node = CanonicalTarget::node();
        let deno: CanonicalTarget = "wasm32-deno-unknown-wasm".parse().unwrap();
        let browser = CanonicalTarget::browser();
        let vendor_node = WhenPredicate { key: "vendor".into(), value: "node".into() };
        assert!(when_item_matches_target(&vendor_node, node));
        assert!(when_item_matches_target(&vendor_node, deno));
        assert!(!when_item_matches_target(&vendor_node, browser));
    }
}
