//! `valkyrie.v` 语料：leetcode_browser_wasm + `std.adaptor.wasm` 文件级 `when`（Random / Fetch）。

use std::path::PathBuf;

use rusty_valkyrie::legion::{CanonicalTarget, PlanRequest, WorkspaceResolver, workspace::planner::BuildRequest};

fn valkyrie_v_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("VALKYRIE_V_ROOT") {
        let path = PathBuf::from(root);
        if path.join("examples").join("leetcode_browser_wasm").join("legion.von").is_file() {
            return Some(path);
        }
    }
    let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../valkyrie.v");
    if sibling.join("examples").join("leetcode_browser_wasm").join("legion.von").is_file() {
        return Some(sibling);
    }
    None
}

fn source_paths_contain(paths: &[PathBuf], fragment: &str) -> bool {
    paths.iter().any(|path| path.to_string_lossy().contains(fragment))
}

#[test]
fn leetcode_browser_wasm_plan_injects_wasm_adaptor_js_random_providers() {
    let Some(valkyrie_v) = valkyrie_v_root() else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };

    let project_dir = valkyrie_v.join("examples").join("leetcode_browser_wasm");
    let workspace = WorkspaceResolver::discover_for_project(&project_dir).expect("discover leetcode example workspace");
    let plan = workspace
        .build_plan(&BuildRequest {
            project_dir: project_dir.clone(),
            target: CanonicalTarget::browser(),
            output_dir: None,
        ..Default::default()
        })
        .expect("leetcode browser wasm plan");

    let wasm_group = plan
        .project
        .semantic_source_groups
        .iter()
        .find(|group| group.name == "std.adaptor.wasm")
        .expect("distribution-default std.adaptor.wasm");
    assert!(
        wasm_group
            .source_files
            .iter()
            .any(|path| path.to_string_lossy().contains("Random.browser.v")),
        "browser when file must be active: {:?}",
        wasm_group.source_files
    );
    assert!(
        source_paths_contain(&wasm_group.source_files, "fetch.browser.v"),
        "browser fetch when file in adaptor group: {:?}",
        wasm_group.source_files
    );
    assert!(
        !source_paths_contain(&wasm_group.source_files, "fetch.wechat.v"),
        "wechat fetch must not appear on browser target: {:?}",
        wasm_group.source_files
    );
    assert!(
        !plan
            .project
            .semantic_source_groups
            .iter()
            .any(|group| group.name == "std.adaptor.wasi"),
        "browser wasm must not pull std.adaptor.wasi package"
    );

    let browser_random_candidates = plan
        .project
        .host_provider_candidates
        .iter()
        .filter(|provider| {
            provider.source_file.to_string_lossy().contains("Random.browser.v")
                && provider.required_capabilities.iter().any(|cap| cap == "js.random")
        })
        .collect::<Vec<_>>();
    assert!(
        !browser_random_candidates.is_empty(),
        "Random.browser.v must register js.random providers: {:?}",
        plan.project.host_provider_candidates
    );

    let selected_random = plan
        .project
        .selected_host_providers
        .iter()
        .filter(|provider| provider.contract.contains("random::Random::"))
        .collect::<Vec<_>>();
    assert!(
        selected_random.iter().any(|provider| provider.contract.contains("with_seed")),
        "entry must select with_seed provider: {:?}",
        selected_random
    );
    assert!(
        selected_random.iter().any(|provider| provider.contract.contains("next_i32")),
        "entry must select next_i32 provider: {:?}",
        selected_random
    );
    assert!(
        selected_random
            .iter()
            .all(|provider| provider.required_capabilities.iter().any(|cap| cap == "js.random")),
        "browser wasm Random providers must be js.random: {:?}",
        selected_random
    );

    let api_plan = rusty_valkyrie::legion::plan(PlanRequest {
        project_dir: project_dir.clone(),
        target: CanonicalTarget::browser(),
        output_dir: None,
    })
    .expect("api plan");
    assert!(api_plan.selected_host_provider_count >= 2);
    assert!(api_plan.host_required_capabilities.iter().any(|cap| cap == "js.random"));
}

#[test]
fn leetcode_miniprogram_wasm_plan_selects_wechat_when_files() {
    let Some(valkyrie_v) = valkyrie_v_root() else {
        eprintln!("skip: valkyrie.v not found");
        return;
    };
    let project_dir = valkyrie_v.join("examples").join("leetcode_miniprogram_wasm");
    if !project_dir.join("legion.von").is_file() {
        eprintln!("skip: leetcode_miniprogram_wasm example missing");
        return;
    }
    let target: CanonicalTarget = "wasm32-unknown-miniprogram-wasm".parse().expect("wechat wasm");
    let workspace = WorkspaceResolver::discover_for_project(&project_dir).expect("discover miniprogram leetcode");
    let plan = workspace
        .build_plan(&BuildRequest { project_dir: project_dir.clone(), target, output_dir: None,
        ..Default::default()
        })
        .expect("leetcode miniprogram wasm plan");

    let wasm_group = plan
        .project
        .semantic_source_groups
        .iter()
        .find(|group| group.name == "std.adaptor.wasm")
        .expect("std.adaptor.wasm group");
    assert!(
        source_paths_contain(&wasm_group.source_files, "Random.wechat.v"),
        "wechat random when file: {:?}",
        wasm_group.source_files
    );
    assert!(
        !wasm_group.source_files.iter().any(|p| p.to_string_lossy().contains("Random.browser.v")),
        "browser random must be filtered: {:?}",
        wasm_group.source_files
    );
    assert!(source_paths_contain(&wasm_group.source_files, "fetch.wechat.v"));
    assert!(!source_paths_contain(&wasm_group.source_files, "fetch.browser.v"));

    let selected_random = plan
        .project
        .selected_host_providers
        .iter()
        .filter(|p| p.contract.contains("random::Random::"))
        .collect::<Vec<_>>();
    assert!(
        selected_random.iter().all(|p| p.required_capabilities.iter().any(|c| c == "js.random")),
        "miniprogram Random providers: {:?}",
        selected_random
    );
}
