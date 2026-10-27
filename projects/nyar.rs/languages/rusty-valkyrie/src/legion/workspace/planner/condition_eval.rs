//! `gate` / `when` 各自求值；**不**在 attribute AST 层组合多个 item，也不根据出现顺序推导 AND/OR。
//!
//! Legion 第一版筛选策略（临时，非语言承诺）：
//! - **when**：0 条恒通过 target 轴；恰好 1 条按 target 求值；2+ 条视为未规定语义 → target 轴不通过。
//! - **gate**：记录事实；无 feature 构建上下文时恒为 `Unknown`，不在此阶段剔除文件或 provider。

use std::collections::BTreeSet;

use nyar::CanonicalTarget;

use super::target_when::when_item_matches_target;
use super::types::{GatePredicate, PlannedHostProvider, WhenPredicate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GateState {
    Enabled,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WhenState {
    Matched,
    Rejected,
    Unknown,
}

pub(crate) fn evaluate_gate_item(gate: &GatePredicate, enabled_features: Option<&BTreeSet<String>>) -> GateState {
    match enabled_features {
        None => GateState::Unknown,
        Some(features) => {
            if gate.key != "feature" {
                return GateState::Unknown;
            }
            if features.contains(&gate.value) {
                GateState::Enabled
            } else {
                GateState::Disabled
            }
        }
    }
}

pub(crate) fn evaluate_when_item(when: &WhenPredicate, target: CanonicalTarget) -> WhenState {
    if when_item_matches_target(when, target) {
        WhenState::Matched
    } else {
        WhenState::Rejected
    }
}

/// 文件级 / provider 的 target 轴：仅处理 0 或 1 个 `when` item。
pub(crate) fn target_axis_passes(whens: &[WhenPredicate], target: CanonicalTarget) -> bool {
    match whens.len() {
        0 => true,
        1 => when_item_matches_target(&whens[0], target),
        _ => false,
    }
}

/// 文件级 / provider 的 gate 轴：0 条 gate 恒通过；有 gate 时每条都必须为 Enabled。
///
/// enabled_features 为 None 时跳过 gate 检查（旧兼容语义）；
/// 显式传 Some(...) 时，空集视为无任何 feature 启用，所有 feature gate 返回 Disabled。
/// 多 gate 当前采取 "全部 Enabled 才通过" 的保守语义（避免隐式 OR）。
pub(crate) fn gate_axis_passes(gates: &[GatePredicate], enabled_features: Option<&BTreeSet<String>>) -> bool {
    if gates.is_empty() {
        return true;
    }
    let Some(features) = enabled_features else {
        return true; // 未指定 feature set 时跳过 gate 检查（旧签名兼容）
    };
    gates.iter().all(|gate| evaluate_gate_item(gate, Some(features)) == GateState::Enabled)
}

pub(crate) fn provider_passes_target_axis(provider: &PlannedHostProvider, target: CanonicalTarget) -> bool {
    target_axis_passes(&provider.when_attributes, target)
}

pub(crate) fn provider_passes_gate_axis(provider: &PlannedHostProvider, enabled_features: Option<&BTreeSet<String>>) -> bool {
    gate_axis_passes(&provider.gate_attributes, enabled_features)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar::CanonicalTarget;

    #[test]
    fn two_when_items_fail_target_axis_without_defined_combine_rule() {
        let whens = vec![
            WhenPredicate { key: "vendor".into(), value: "browser".into() },
            WhenPredicate { key: "vendor".into(), value: "wechat".into() },
        ];
        assert!(!target_axis_passes(&whens, CanonicalTarget::wasm()));
    }
}
