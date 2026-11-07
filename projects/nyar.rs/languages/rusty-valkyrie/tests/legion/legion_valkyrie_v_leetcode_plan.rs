//! `valkyrie.v` 语料：leetcode_browser_wasm + `std.adaptor.wasm` 文件级 `when`（Random / Fetch）。

use std::path::PathBuf;

use rusty_valkyrie::legion::{CanonicalTarget, WorkspaceResolver, workspace::planner::BuildRequest};

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
    let Some(valkyrie_v) = valkyrie_v_root()
    else {
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
        wasm_group.source_files.iter().any(|path| path.to_string_lossy().contains("Random.browser.v")),
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
        !plan.project.semantic_source_groups.iter().any(|group| group.name == "std.adaptor.wasi"),
        "browser wasm must not pull std.adaptor.wasi package"
    );

    let browser_random_candidates = plan
        .project
        .host_provider_candidates
        .iter()
        .filter(|provider| provider.source_file.to_string_lossy().contains("Random.browser.v"))
        .collect::<Vec<_>>();
    assert!(
        !browser_random_candidates.is_empty(),
        "Random.browser.v must register browser glue providers: {:?}",
        plan.project.host_provider_candidates
    );

    // 新合同：browser glue 只声明 `[host_provider(std::random::entropy)]`，`js.random`
    // 来自它体内真实调用的 `[js_builtin("random")]` glue，必须经调用边闭包进入能力表。
    let browser_entropy = browser_random_candidates
        .iter()
        .find(|provider| provider.symbol.ends_with("host_entropy"))
        .expect("Random.browser.v must declare host_entropy provider");
    assert!(
        browser_entropy.contract.contains("random::entropy"),
        "host_entropy contract must stay on the new `std.random.entropy` contract: {:?}",
        browser_entropy
    );
    assert!(
        browser_entropy.required_capabilities.iter().any(|capability| capability == "js.random"),
        "host_entropy must inherit `js.random` from its called `[js_builtin]` glue: {:?}",
        browser_entropy.required_capabilities
    );
    assert!(
        browser_entropy.foreign_bindings.is_empty(),
        "host_entropy itself declares no foreign binding; capability must come from the body call edge: {:?}",
        browser_entropy.foreign_bindings
    );
}

#[test]
fn leetcode_miniprogram_wasm_plan_selects_wechat_when_files() {
    let Some(valkyrie_v) = valkyrie_v_root()
    else {
        eprintln!("skip: valkyrie.v not found");
        return;
    };
    let project_dir = valkyrie_v.join("examples").join("leetcode_miniprogram_wasm");
    if !project_dir.join("legion.von").is_file() {
        eprintln!("skip: leetcode_miniprogram_wasm example missing");
        return;
    }
    let target = CanonicalTarget::wechat();
    let workspace = WorkspaceResolver::discover_for_project(&project_dir).expect("discover miniprogram leetcode");
    let plan = workspace
        .build_plan(&BuildRequest { project_dir: project_dir.clone(), target, output_dir: None, ..Default::default() })
        .expect("leetcode miniprogram wasm plan");

    let wasm_group = plan.project.semantic_source_groups.iter().find(|group| group.name == "std.adaptor.wasm").expect("std.adaptor.wasm group");
    assert!(source_paths_contain(&wasm_group.source_files, "Random.wechat.v"), "wechat random when file: {:?}", wasm_group.source_files);
    assert!(
        !wasm_group.source_files.iter().any(|p| p.to_string_lossy().contains("Random.browser.v")),
        "browser random must be filtered: {:?}",
        wasm_group.source_files
    );
    assert!(source_paths_contain(&wasm_group.source_files, "fetch.wechat.v"));
    assert!(!source_paths_contain(&wasm_group.source_files, "fetch.browser.v"));

    let wechat_entropy = plan
        .project
        .host_provider_candidates
        .iter()
        .find(|provider| provider.source_file.to_string_lossy().contains("Random.wechat.v") && provider.symbol.ends_with("host_entropy"))
        .expect("Random.wechat.v must declare host_entropy provider");
    assert!(
        wechat_entropy.contract.contains("random::entropy"),
        "wechat host_entropy contract must stay on the new `std.random.entropy` contract: {:?}",
        wechat_entropy
    );
    assert!(
        wechat_entropy.required_capabilities.iter().any(|capability| capability == "js.random"),
        "wechat host_entropy must inherit `js.random` from its called `[js_builtin]` glue: {:?}",
        wechat_entropy.required_capabilities
    );
}
