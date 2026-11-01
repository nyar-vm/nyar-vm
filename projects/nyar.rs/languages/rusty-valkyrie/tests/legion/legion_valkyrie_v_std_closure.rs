//! `valkyrie.v` 语料：`core` 并入 `std` 后，纯数值工程显式声明 workspace `std` 依赖，
//! 其需求闭包必须只经 `std` 解析，且不得拖入 IO/网络 host binding。

use std::path::PathBuf;

use rusty_valkyrie::legion::{CanonicalTarget, PlanRequest, plan};

fn valkyrie_v_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("VALKYRIE_V_ROOT") {
        let path = PathBuf::from(root);
        if path.join("examples").join("usize-cmp-smoke").join("legion.von").is_file() {
            return Some(path);
        }
    }
    let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../valkyrie.v");
    if sibling.join("examples").join("usize-cmp-smoke").join("legion.von").is_file() {
        return Some(sibling);
    }
    None
}

/// IO/网络 capability 与外部绑定标记；纯数值闭包命中任何一个都说明 `std:true` 拖进了宿主 IO。
const IO_NETWORK_MARKERS: &[&str] = &["fetch", "random", "http", "net.", "socket", "fs.", "storage", "websocket"];

fn hits_io_or_network(text: &str) -> bool {
    let lowered = text.to_lowercase();
    IO_NETWORK_MARKERS.iter().any(|marker| lowered.contains(marker))
}

/// 正：`usize-cmp-smoke`（纯数值、显式 `std` workspace 依赖）以 clr target 成计划，
/// 闭包含 `std` 组、不含任何 `core` 依赖键，且已选 provider 的 capability/外部绑定无 IO/网络。
#[test]
fn usize_cmp_smoke_std_closure_resolves_without_core_or_io_bindings() {
    let Some(valkyrie_v) = valkyrie_v_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let project_dir = valkyrie_v.join("examples").join("usize-cmp-smoke");
    let plan = plan(PlanRequest { project_dir, target: CanonicalTarget::clr(), output_dir: None }).expect("usize-cmp-smoke plan");

    assert!(
        plan.groups.iter().any(|group| group.dependency_key == "std"),
        "std:true 必须解析出 `std` 语义组：{:?}",
        plan.groups.iter().map(|group| &group.dependency_key).collect::<Vec<_>>()
    );
    assert!(
        plan.groups.iter().all(|group| !group.dependency_key.contains("core")),
        "core 已并入 std，闭包不得再出现 core 依赖键：{:?}",
        plan.groups.iter().map(|group| &group.dependency_key).collect::<Vec<_>>()
    );

    for provider in &plan.selected_host_providers {
        for capability in &provider.required_capabilities {
            assert!(!hits_io_or_network(capability), "纯数值闭包不得要求 IO/网络 capability：{capability}（provider {}）", provider.symbol);
        }
        for binding in &provider.foreign_bindings {
            assert!(!hits_io_or_network(binding), "纯数值闭包不得携带 IO/网络外部绑定：{binding}（provider {}）", provider.symbol);
        }
    }
    for capability in &plan.host_required_capabilities {
        assert!(!hits_io_or_network(capability), "闭包 capability 汇总不得含 IO/网络：{capability}");
    }
}
