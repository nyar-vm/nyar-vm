//! `valkyrie.v` 语料：leetcode_browser_wasm + `std.adaptor.wasm` Random 文件级 `when`。

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
            target: CanonicalTarget::wasm(),
            output_dir: None,
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
        "browser cfg file must be active: {:?}",
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
        target: CanonicalTarget::wasm(),
        output_dir: None,
    })
    .expect("api plan");
    assert!(api_plan.selected_host_provider_count >= 2);
    assert!(api_plan.host_required_capabilities.iter().any(|cap| cap == "js.random"));
}
