//! `legion test` 目标解析结构化回归测试。
//!
//! 锁定事实：未标注 target 默认产品别名 `legion`（内置 OVM 执行 lane）；
//! 显式 target 的 runner 家族一律由 `CanonicalTarget::to_profile(None).runner_family()`
//! 结构化推导，完整三元组不得按标签字符串误判，jvm/clr 不得静默换成其他 lane。

use rusty_valkyrie::legion::{
    RunnerFamily,
    cmds::test_engine::{parse_target_label, resolve_test_targets},
};

#[test]
fn default_target_is_legion_product_alias() {
    assert_eq!(resolve_test_targets(None), vec!["legion".to_string()]);
    assert_eq!(resolve_test_targets(Some("")), vec!["legion".to_string()]);
}

#[test]
fn all_expands_to_four_lanes() {
    let targets = resolve_test_targets(Some("all"));
    for expected in ["legion", "clr", "jvm", "node"] {
        assert!(targets.iter().any(|target| target == expected), "{targets:?}");
    }
}

#[test]
fn legion_alias_resolves_to_builtin_ovm_family() {
    let canonical = parse_target_label("legion").expect("`legion` 必须解析为合法目标");
    assert_eq!(canonical.to_profile(None).runner_family(), RunnerFamily::Ovm);
}

#[test]
fn explicit_backend_labels_resolve_to_their_own_runner_families() {
    assert_eq!(parse_target_label("ovm").unwrap().to_profile(None).runner_family(), RunnerFamily::Ovm);
    assert_eq!(parse_target_label("jvm").unwrap().to_profile(None).runner_family(), RunnerFamily::Jvm);
    assert_eq!(parse_target_label("clr").unwrap().to_profile(None).runner_family(), RunnerFamily::Clr);
    assert_eq!(parse_target_label("node").unwrap().to_profile(None).runner_family(), RunnerFamily::Node);
}

#[test]
fn full_triple_is_derived_structurally_not_by_label_matching() {
    // 完整三元组含 `wasm32` 片段；若按标签字符串硬编码会把 node 宿主误判为内置 Ovm。
    let canonical = parse_target_label("wasm32-node-unknown").expect("完整三元组必须可解析");
    let family = canonical.to_profile(None).runner_family();
    assert_eq!(family, RunnerFamily::Node);
    assert_ne!(family, RunnerFamily::Ovm);
}

#[test]
fn contrast_lanes_never_collapse_into_delivery_or_builtin_lanes() {
    for label in ["jvm", "clr"] {
        let family = parse_target_label(label).unwrap().to_profile(None).runner_family();
        assert_ne!(family, RunnerFamily::Ovm, "`{label}` 被静默换成了内置 OVM lane");
        assert_ne!(family, RunnerFamily::Node, "`{label}` 被静默换成了 wasm 交付 lane");
    }
}
