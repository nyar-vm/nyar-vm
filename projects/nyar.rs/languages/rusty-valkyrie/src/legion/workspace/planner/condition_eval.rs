//! `gate` / `when` 各自求值；**不**在 attribute AST 层组合多个 item，也不根据出现顺序推导 AND/OR。
//!
//! Legion 第一版筛选策略（临时，非语言承诺）：
//! - **when**：0 条恒通过 target 轴；恰好 1 条按 target 求值；2+ 条视为未规定语义 → target 轴不通过。
//! - **gate**：记录事实；缺少 feature 构建上下文或条件 key 未知时得到 `Unknown`，按不通过处理（fail-closed）。

use std::collections::BTreeSet;

use nyar::CanonicalTarget;

use super::{
    target_when::when_item_matches_target,
    types::{GatePredicate, PlannedHostProvider, WhenPredicate},
};

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
            if features.contains(&gate.value) { GateState::Enabled } else { GateState::Disabled }
        }
    }
}

pub(crate) fn evaluate_when_item(when: &WhenPredicate, target: CanonicalTarget) -> WhenState {
    if when_item_matches_target(when, target) { WhenState::Matched } else { WhenState::Rejected }
}

/// 文件级 / provider 的 target 轴：仅处理 0 或 1 个 `when` item。
pub(crate) fn target_axis_passes(whens: &[WhenPredicate], target: CanonicalTarget) -> bool {
    match whens.len() {
        0 => true,
        1 => when_item_matches_target(&whens[0], target),
        _ => false,
    }
}

/// 文件级 / provider 的 gate 轴：0 条 gate 恒通过；有 gate 时仅当每条都被确认 `Enabled` 才通过。
///
/// fail-closed：`enabled_features == None`（无 feature 构建上下文）与未知 gate key 都得到
/// `Unknown`，按不通过处理；显式传入空集表示“未启用任何 feature”，因此所有 feature gate
/// 为 `Disabled`。
/// 多 gate 当前采取 "全部 Enabled 才通过" 的保守语义（避免隐式 OR）。
pub(crate) fn gate_axis_passes(gates: &[GatePredicate], enabled_features: Option<&BTreeSet<String>>) -> bool {
    gate_axis_state(gates, enabled_features) == GateState::Enabled
}

/// 聚合 gate 轴的诊断状态：任一 `Disabled` 立即为 `Disabled`；否则任一 `Unknown` 为 `Unknown`。
///
/// 只有空 gate 或全部 `Enabled` 才返回 `Enabled`。调用方不得把 `Unknown` 当作通过。
fn gate_axis_state(gates: &[GatePredicate], enabled_features: Option<&BTreeSet<String>>) -> GateState {
    let mut state = GateState::Enabled;
    for gate in gates {
        match evaluate_gate_item(gate, enabled_features) {
            GateState::Enabled => {}
            GateState::Disabled => return GateState::Disabled,
            GateState::Unknown => state = GateState::Unknown,
        }
    }
    state
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

    fn feature_gate(value: &str) -> GatePredicate {
        GatePredicate { key: "feature".into(), value: value.into() }
    }

    fn features(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn two_when_items_fail_target_axis_without_defined_combine_rule() {
        let whens = vec![
            WhenPredicate { key: "vendor".into(), value: "browser".into() },
            WhenPredicate { key: "vendor".into(), value: "wechat".into() },
        ];
        assert!(!target_axis_passes(&whens, CanonicalTarget::browser()));
    }

    #[test]
    fn gate_without_feature_context_is_unknown_and_fails_closed() {
        let gates = vec![feature_gate("fetch")];
        assert_eq!(gate_axis_state(&gates, None), GateState::Unknown);
        assert!(!gate_axis_passes(&gates, None));
    }

    #[test]
    fn gate_with_explicit_empty_feature_set_is_disabled() {
        let gates = vec![feature_gate("fetch")];
        let enabled = features(&[]);
        assert_eq!(gate_axis_state(&gates, Some(&enabled)), GateState::Disabled);
        assert!(!gate_axis_passes(&gates, Some(&enabled)));
    }

    #[test]
    fn gate_with_enabled_feature_passes() {
        let gates = vec![feature_gate("fetch")];
        let enabled = features(&["fetch"]);
        assert_eq!(gate_axis_state(&gates, Some(&enabled)), GateState::Enabled);
        assert!(gate_axis_passes(&gates, Some(&enabled)));
    }

    #[test]
    fn gate_with_unknown_key_is_unknown_and_fails_closed() {
        let gates = vec![GatePredicate { key: "vendor".into(), value: "browser".into() }];
        let enabled = features(&["fetch"]);
        assert_eq!(gate_axis_state(&gates, Some(&enabled)), GateState::Unknown);
        assert!(!gate_axis_passes(&gates, Some(&enabled)));
        assert!(!gate_axis_passes(&gates, None));
    }

    #[test]
    fn no_gate_passes_even_without_feature_context() {
        assert_eq!(gate_axis_state(&[], None), GateState::Enabled);
        assert!(gate_axis_passes(&[], None));
    }

    #[test]
    fn unknown_gate_outranks_enabled_gate_in_axis_state() {
        let gates = vec![feature_gate("fetch"), GatePredicate { key: "vendor".into(), value: "browser".into() }];
        let enabled = features(&["fetch"]);
        assert_eq!(gate_axis_state(&gates, Some(&enabled)), GateState::Unknown);
        assert!(!gate_axis_passes(&gates, Some(&enabled)));
    }
}
