//! LeetCode / harness：execution manifest、`legion test` 与内嵌 OVM 调用（不经 argv、不定位外部 CLI）。

use std::{fs, path::PathBuf};

use napi::bindgen_prelude::*;
use napi_derive::napi;
use odin_vm::{OdinVm, json_bridge};
use rusty_valkyrie::legion::{
    CanonicalTarget,
    cmds::test::{TestArgs, run as run_test},
};

const EXECUTION_MANIFEST_FILE: &str = "run-contracts.txt";

/// execution manifest 摘要（JSON 友好）。
#[napi(object)]
pub struct LegionExecutionManifestView {
    /// schema 版本。
    pub schema_version: u32,
    /// canonical target 字符串。
    pub target: String,
    /// 语义闭包哈希。
    pub semantic_closure_hash: Option<String>,
    /// 规范表面哈希。
    pub canonical_surface_hash: Option<String>,
    /// host provider 绑定哈希。
    pub host_provider_binding_hash: Option<String>,
    /// 首个 run contract 物理入口。
    pub primary_physical_entry: Option<String>,
    /// 首个 run contract 逻辑入口。
    pub primary_logical_entry: Option<String>,
}

/// 产物新鲜度检查结果。
#[napi(object)]
pub struct LegionArtifactFreshness {
    /// 是否可信复用。
    pub fresh: bool,
    /// 失败原因。
    pub reason: Option<String>,
}

/// `legion test` 结构化结果。
#[napi(object)]
pub struct LegionTestOutcome {
    /// 是否全部通过。
    pub ok: bool,
    /// 失败说明。
    pub error: Option<String>,
    /// 通过数。
    pub passed: u32,
    /// 失败数。
    pub failed: u32,
    /// 跳过数。
    pub skipped: u32,
}

fn read_manifest(output_dir: &std::path::Path) -> Option<rusty_valkyrie::legion::cmds::run::ExecutionManifest> {
    rusty_valkyrie::legion::cmds::run::ExecutionManifest::read_from_output_dir(output_dir).ok().flatten()
}

/// 读取 `run-contracts.txt` 摘要。
#[napi]
pub fn legion_read_execution_manifest(output_dir: String) -> Option<LegionExecutionManifestView> {
    let path = PathBuf::from(&output_dir);
    let manifest = read_manifest(&path)?;
    let contract = manifest.run_contracts.first();
    let evidence = manifest.compile_evidence.as_ref();
    Some(LegionExecutionManifestView {
        schema_version: manifest.schema_version,
        target: manifest.target,
        semantic_closure_hash: evidence.map(|e| e.semantic_closure_hash.clone()),
        canonical_surface_hash: evidence.map(|e| e.canonical_surface_hash.clone()),
        host_provider_binding_hash: evidence.map(|e| e.host_provider_binding_hash.clone()),
        primary_physical_entry: contract.map(|c| c.physical_entry.clone()),
        primary_logical_entry: contract.map(|c| c.logical_entry.clone()),
    })
}

/// 判断输出目录中的 execution manifest 是否匹配目标且含编译证据。
#[napi]
pub fn legion_check_artifact_fresh(output_dir: String, expected_target: String) -> LegionArtifactFreshness {
    let path = PathBuf::from(&output_dir);
    let manifest_path = path.join(EXECUTION_MANIFEST_FILE);
    if !manifest_path.is_file() {
        return LegionArtifactFreshness { fresh: false, reason: Some("missing run-contracts.txt".into()) };
    }
    let manifest = match read_manifest(&path) {
        Some(m) => m,
        None => {
            return LegionArtifactFreshness { fresh: false, reason: Some("failed to parse run-contracts.txt".into()) };
        }
    };
    if manifest.target != expected_target {
        return LegionArtifactFreshness {
            fresh: false,
            reason: Some(format!("manifest target `{}` != expected `{}`", manifest.target, expected_target)),
        };
    }
    if manifest.compile_evidence.is_none() {
        return LegionArtifactFreshness { fresh: false, reason: Some("manifest missing compile_evidence".into()) };
    }
    LegionArtifactFreshness { fresh: true, reason: None }
}

/// 运行 `legion test`（结构化；不经 argv）。
#[napi]
pub fn legion_test(project_dir: String, target: Option<String>) -> LegionTestOutcome {
    let target_label = target.unwrap_or_else(|| "node".into());
    let parsed_target = match CanonicalTarget::parse(&target_label) {
        Ok(t) => t.to_string(),
        Err(error) => {
            return LegionTestOutcome { ok: false, error: Some(error.to_string()), passed: 0, failed: 0, skipped: 0 };
        }
    };
    let worker = std::thread::Builder::new()
        .name("legion-test".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            run_test(&TestArgs {
                project_dir: PathBuf::from(project_dir),
                filter: None,
                target: Some(parsed_target),
                runner: Vec::new(),
                verbose: false,
                standalone: false,
            })
        })
        .expect("failed to spawn legion test thread");
    match worker.join().expect("legion test thread panicked") {
        Ok(code) => {
            let success = code == std::process::ExitCode::SUCCESS;
            LegionTestOutcome {
                ok: success,
                error: if success { None } else { Some("legion test reported failures".into()) },
                passed: if success { 1 } else { 0 },
                failed: if success { 0 } else { 1 },
                skipped: 0,
            }
        }
        Err(error) => LegionTestOutcome { ok: false, error: Some(error.to_string()), passed: 0, failed: 1, skipped: 0 },
    }
}

/// Legion 内嵌 OVM：列出 `.nyar` 导出（与 `legion test` / 元编程执行同栈）。
#[napi]
pub fn legion_list_ovm_exports(module: String) -> Result<Vec<String>> {
    let bytes = fs::read(&module).map_err(|error| Error::from_reason(format!("read module: {error}")))?;
    let loaded = OdinVm::new()
        .load(&bytes)
        .map_err(|error| Error::from_reason(format!("load module: {error}")))?;
    Ok(loaded.exports.iter().map(|export| export.symbol_name.clone()).collect())
}

/// Legion 内嵌 OVM：调用导出并返回 JSON（`args_json` 为 JSON 数组）。
#[napi]
pub fn legion_invoke_ovm_export(module: String, entry: String, args_json: String) -> Result<String> {
    let bytes = fs::read(&module).map_err(|error| Error::from_reason(format!("read module: {error}")))?;
    let mut vm = OdinVm::new();
    let loaded = vm
        .load(&bytes)
        .map_err(|error| Error::from_reason(format!("load module: {error}")))?;
    let args = json_bridge::parse_call_args_json_with_heap(&args_json, vm.heap_mut())
        .map_err(|error| Error::from_reason(format!("parse args-json: {error}")))?;
    let result = vm
        .run(&loaded, &entry, args)
        .map_err(|error| Error::from_reason(format!("run entry `{entry}`: {error}")))?;
    let json = json_bridge::value_to_json_with_heap(&result, Some(vm.heap()))
        .map_err(|error| Error::from_reason(format!("serialize result: {error}")))?;
    serde_json::to_string(&json).map_err(|error| Error::from_reason(format!("encode result json: {error}")))
}
