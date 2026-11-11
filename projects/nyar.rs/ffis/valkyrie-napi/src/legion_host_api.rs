//! LeetCode / harness：execution manifest、`legion test` 与内嵌 OVM 调用（不经 argv、不定位外部 CLI）。

use std::{fs, path::Path, path::PathBuf};

use napi::bindgen_prelude::*;
use napi_derive::napi;
use odin_bytecode::{OvmAbiSidecar, ovm_abi_sidecar_file};
use odin_vm::{OdinVm, json_bridge};
use rusty_valkyrie::legion::{
    CanonicalTarget,
    cmds::error::LegionResult,
    cmds::run::{ManifestVerificationContext, ManifestVerificationLevel, ManifestVerdict, collect_execution_input_digests, expected_resolver_compile_evidence},
    cmds::test::{TestArgs, run as run_test},
    workspace::planner::{BuildRequest, WorkspaceResolver},
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
    /// 产出该构建的编译器构建身份（manifest 工具链指纹 `compiler=` 段）；
    /// 旧构建 manifest 指纹缺少该段时为 `null`。
    pub compiler_build_identity: Option<String>,
}

/// 产物新鲜度检查结果。
#[napi(object)]
pub struct LegionArtifactFreshness {
    /// 是否可信复用；只有 `level == "fully-verified"` 才为 `true`。
    pub fresh: bool,
    /// 失败原因。
    pub reason: Option<String>,
    /// 权威校验级别：`fully-verified` / `artifact-self-consistent` / `stale`。
    /// 正式复用消费者只能对 `fully-verified` 放行。
    pub level: String,
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
        compiler_build_identity: rusty_valkyrie::legion::cache::compiler_build_id_from_fingerprint(&manifest.toolchain_fingerprint).map(str::to_string),
    })
}

/// 本 addon 内嵌的编译器构建身份（与 `toolchain_fingerprint()` 的 `compiler=` 段一致）。
#[napi]
pub fn legion_compiler_build_identity() -> String {
    rusty_valkyrie::legion::cache::compiler_build_id().to_string()
}

/// 本 addon 的完整 toolchain fingerprint（缓存键组成部分），供消费者交叉核对。
#[napi]
pub fn legion_toolchain_fingerprint() -> String {
    rusty_valkyrie::legion::cache::toolchain_fingerprint()
}

const FRESHNESS_LEVEL_FULLY: &str = "fully-verified";
const FRESHNESS_LEVEL_SELF_CONSISTENT: &str = "artifact-self-consistent";
const FRESHNESS_LEVEL_STALE: &str = "stale";

fn stale_freshness(reason: String) -> LegionArtifactFreshness {
    LegionArtifactFreshness { fresh: false, reason: Some(reason), level: FRESHNESS_LEVEL_STALE.into() }
}

fn verdict_to_freshness(verdict: LegionResult<ManifestVerdict>) -> LegionArtifactFreshness {
    match verdict {
        Ok(ManifestVerdict::Verified(ManifestVerificationLevel::FullyVerified)) => {
            LegionArtifactFreshness { fresh: true, reason: None, level: FRESHNESS_LEVEL_FULLY.into() }
        }
        Ok(ManifestVerdict::Verified(ManifestVerificationLevel::ArtifactSelfConsistent)) => LegionArtifactFreshness {
            fresh: false,
            reason: Some("artifact self-consistent only: inputs not verified against current workspace; pass projectDir for formal reuse".into()),
            level: FRESHNESS_LEVEL_SELF_CONSISTENT.into(),
        },
        Ok(ManifestVerdict::Stale(reason)) => stale_freshness(reason),
        Err(error) => stale_freshness(format!("manifest verification failed: {error}")),
    }
}

/// 判断输出目录中的 execution manifest 是否可信复用：target 匹配后，走 Rust 侧唯一
/// 权威核心 [`ExecutionManifest::verify`] 做真实摘要校验——schema/合同版本与工具链
/// 指纹（含 compiler build identity）须与当前构建一致，输入/产物摘要字段须齐全，
/// 落盘产物字节须与摘要一致，compile evidence 旁文件须与 manifest 记录一致。
///
/// 缺字段、摘要不符、产物消失一律 `fresh:false` 并带 reason，不得只看 manifest 存在。
///
/// `project_dir` 提供时，核心会重算当前 workspace 输入摘要与 resolver compile
/// evidence，达到 `fully-verified`；不提供时最高只能达到 `artifact-self-consistent`，
/// 正式复用消费者不得放行。
#[napi]
pub fn legion_check_artifact_fresh(output_dir: String, expected_target: String, project_dir: Option<String>) -> LegionArtifactFreshness {
    let path = PathBuf::from(&output_dir);
    let manifest_path = path.join(EXECUTION_MANIFEST_FILE);
    if !manifest_path.is_file() {
        return stale_freshness("missing run-contracts.txt".into());
    }
    let manifest = match read_manifest(&path) {
        Some(m) => m,
        None => {
            return stale_freshness("failed to parse run-contracts.txt".into());
        }
    };
    if manifest.target != expected_target {
        return stale_freshness(format!("manifest target `{}` != expected `{}`", manifest.target, expected_target));
    }
    let verdict = match &project_dir {
        Some(dir) => {
            let target = match CanonicalTarget::parse(&expected_target) {
                Ok(parsed) => parsed,
                Err(error) => return stale_freshness(format!("invalid expected target `{expected_target}`: {error}")),
            };
            let project_path = PathBuf::from(dir);
            let workspace = match WorkspaceResolver::discover_for_project(&project_path) {
                Ok(w) => w,
                Err(error) => return stale_freshness(format!("workspace discovery failed for `{dir}`: {error}")),
            };
            let request = BuildRequest {
                project_dir: project_path,
                target,
                output_dir: Some(path.clone()),
                ..Default::default()
            };
            let plan = match workspace.build_plan(&request) {
                Ok(p) => p,
                Err(error) => return stale_freshness(format!("build plan failed for `{dir}`: {error}")),
            };
            let inputs = match collect_execution_input_digests(&plan) {
                Ok(v) => v,
                Err(error) => return stale_freshness(format!("input digests: {error}")),
            };
            let expected = match expected_resolver_compile_evidence(&plan) {
                Ok(v) => v,
                Err(error) => return stale_freshness(format!("expected evidence: {error}")),
            };
            let plan_target = plan.project.build_target.target.to_string();
            let context = ManifestVerificationContext {
                expected_project: Some((plan.project.name.as_str(), plan_target.as_str(), plan.project.artifact_kind)),
                current_inputs: Some(&inputs),
                expected_evidence: Some(&expected),
            };
            manifest.verify(&path, &context)
        }
        None => manifest.verify_against_output_dir(&path),
    };
    verdict_to_freshness(verdict)
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

/// Legion 内嵌 OVM：列出 `.odin` 导出（与 `legion test` / 元编程执行同栈）。
#[napi]
pub fn legion_list_ovm_exports(module: String) -> Result<Vec<String>> {
    let bytes = fs::read(&module).map_err(|error| Error::from_reason(format!("read module: {error}")))?;
    let loaded = OdinVm::new().load(&bytes).map_err(|error| Error::from_reason(format!("load module: {error}")))?;
    Ok(loaded.exports.iter().map(|export| export.symbol_name.clone()).collect())
}

/// 读取并校验产物旁的 ABI sidecar；缺失或哈希不符都失败闭合。
///
/// 结构物化只允许按“与这枚 `.odin` 字节一一对应”的合同进行：sidecar 缺失、
/// 解析失败或产物哈希不一致时宁可拒绝调用，也不按猜测的布局解释堆内存。
fn load_ovm_abi_sidecar(module: &str, artifact_bytes: &[u8]) -> Result<OvmAbiSidecar> {
    let module_path = Path::new(module);
    let artifact = module_path.file_name().and_then(|name| name.to_str()).unwrap_or(module);
    let sidecar_path = module_path.with_file_name(ovm_abi_sidecar_file(artifact));
    let sidecar_bytes = fs::read(&sidecar_path).map_err(|error| {
        Error::from_reason(format!(
            "OVM ABI 合同缺失：`{}`（{error}）；拒绝在无合同的情况下物化结构参数/返回值",
            sidecar_path.display()
        ))
    })?;
    let sidecar: OvmAbiSidecar = serde_json::from_slice(&sidecar_bytes)
        .map_err(|error| Error::from_reason(format!("解析 OVM ABI 合同 `{}`: {error}", sidecar_path.display())))?;
    sidecar
        .verify_against(artifact_bytes)
        .map_err(|error| Error::from_reason(format!("校验 OVM ABI 合同 `{}`: {error}", sidecar_path.display())))?;
    Ok(sidecar)
}

/// Legion 内嵌 OVM：调用导出并返回 JSON（`args_json` 为 JSON 数组）。
#[napi]
pub fn legion_invoke_ovm_export(module: String, entry: String, args_json: String) -> Result<String> {
    let bytes = fs::read(&module).map_err(|error| Error::from_reason(format!("read module: {error}")))?;
    let sidecar = load_ovm_abi_sidecar(&module, &bytes)?;
    let export = sidecar.contract.export(&entry).ok_or_else(|| {
        Error::from_reason(format!(
            "OVM ABI 合同没有导出 `{entry}`；拒绝按未声明的结构物化参数/返回值"
        ))
    })?;
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).map_err(|error| Error::from_reason(format!("load module: {error}")))?;
    let args = json_bridge::parse_call_args_json_with_contract(&args_json, export, vm.heap_mut())
        .map_err(|error| Error::from_reason(format!("parse args-json: {error}")))?;
    let result = vm.run(&loaded, &entry, args).map_err(|error| Error::from_reason(format!("run entry `{entry}`: {error}")))?;
    let json = json_bridge::value_to_json_with_contract(&result, &export.returns, vm.heap())
        .map_err(|error| Error::from_reason(format!("serialize result: {error}")))?;
    serde_json::to_string(&json).map_err(|error| Error::from_reason(format!("encode result json: {error}")))
}
