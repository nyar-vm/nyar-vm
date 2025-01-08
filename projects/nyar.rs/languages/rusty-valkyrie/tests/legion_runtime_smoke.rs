mod support;

use std::path::{Path, PathBuf};
use support::runtime_fixture::{can_run_all_required_commands, collect_runtime_fixture_cases, regenerate_enabled, verify_runtime_fixture};

// 夹具 manifest 显式声明 `core: false` / `std: false`，运算符只能绑定到 per-target
// core adaptor 的源级声明。本仓没有适配层源码，因此 `infix <` 等调用必然未解析；
// 待适配层接入（从 `valkyrie.v` 解析 target adaptor 组）后再启用这组真实运行验收。
#[ignore = "runtime fixtures 缺少 per-target core adaptor 运算符声明，待适配层接入后启用"]
#[test]
fn runs_runtime_fixtures_across_declared_targets() {
    let fixtures_root = runtime_fixture_root();
    let fixtures = collect_runtime_fixture_cases(&fixtures_root);
    assert!(!fixtures.is_empty(), "no runtime fixtures found under '{}'", fixtures_root.display());

    if !can_run_all_required_commands(&fixtures) {
        return;
    }

    for fixture in &fixtures {
        verify_runtime_fixture(fixture);
    }

    if regenerate_enabled() {
        eprintln!("runtime fixtures regenerated under {}", fixtures_root.display());
    }
}

fn runtime_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("runtime_smoke")
}
