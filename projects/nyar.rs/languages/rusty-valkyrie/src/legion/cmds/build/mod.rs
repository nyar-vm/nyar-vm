// `legion build` 命令。
//
// 这里只负责装配与落盘；目标解释、后端注册和 lowering 均交给 `emitter`。
use crate::{
    compiler::emitter::pipeline::{CompilerBuildContext, compile_source_groups_to_artifacts},
    legion::{
        cmds::error::{LegionError, LegionResult},
        workspace::{
            labeled_report_with_context,
            manifest::{ProjectArtifactKind, ProjectManifest},
            planner::{BuildPlan, BuildRequest, WorkspaceResolver, compiler_host_provider_bindings},
            source_snapshot::compile_source_snapshot,
        },
    },
};
use nyar::{ArtifactContract, ArtifactSet, CanonicalTarget, ClrSuspendStrategy};
use nyar_emitter::DriverRunContract;
use nyar_types::compiler_input::CompilerSourceInput;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use crate::legion::{
    cache::{CompilationCache, cache_root_for, collect_build_bundle, compute_artifact_hash, store_cached_build},
    cmds::{
        project_input::resolve_project_path,
        run::{ExecutionManifest, RunContract},
    },
    script, unity_export, write_von_indented,
};

/// `legion build` 的命令参数。
#[derive(Debug, Clone)]
pub struct BuildArgs {
    /// 项目目录，默认当前目录。
    pub project_dir: PathBuf,
    /// 已冻结的解析目标：平台身份 + 产物/宿主合同。
    ///
    /// `native` / `node` / `python` 占位符必须先经统一目标解析服务
    /// （[`crate::legion::resolve_authoring_target`]）展开成完整事实再进入
    /// 构建；合同不在这里丢失，下游不猜产物形态。
    pub resolved: crate::legion::ResolvedTarget,
    /// 输出目录。
    pub output_dir: Option<PathBuf>,
    /// 输出调试用构建副产物。
    pub debug_artifacts: bool,
}

/// 执行 `legion build`。
///
/// 编译链是表达式深度驱动的递归，主线程默认栈装不下；统一在 `legion-driver` worker 上跑。
pub fn run(args: &BuildArgs) -> LegionResult<ExitCode> {
    let args = args.clone();
    crate::legion::cmds::run_with_driver_stack(move || run_inner(&args))
}

fn run_inner(args: &BuildArgs) -> LegionResult<ExitCode> {
    let project_input = resolve_project_path(&args.project_dir)?;
    let workspace = WorkspaceResolver::discover_for_project(&project_input)?;
    if workspace.is_at_workspace_root(&project_input) {
        return run_all_workspace_members(args);
    }

    let request = BuildRequest {
        project_dir: project_input,
        resolved: args.resolved.clone(),
        output_dir: args.output_dir.clone(),
        ..Default::default()
    };
    let plan = workspace.build_plan(&request)?;

    println!("workspace: {}", crate::legion::cmds::path_for_cli_log(&plan.workspace_root));
    println!("mode: workspace");
    println!("project: {}", plan.project.name);
    println!("target: {}", plan.project.build_target.target);
    if matches!(plan.project.resolved.contract, ArtifactContract::NodeApi | ArtifactContract::Cpython(_)) {
        println!("host contract: {}", plan.project.resolved.contract.as_str());
    }
    println!("output: {}", crate::legion::cmds::path_for_cli_log(&plan.output_dir));
    println!("sources: {}", plan.project.source_files.len());
    println!("host contracts: {}", plan.project.host_contracts.len());
    println!("selected host providers: {}", plan.project.selected_host_providers.len());
    let host_capabilities = {
        let mut capabilities =
            plan.project.selected_host_providers.iter().flat_map(|provider| provider.required_capabilities.iter().cloned()).collect::<Vec<_>>();
        capabilities.sort();
        capabilities.dedup();
        capabilities
    };
    if !host_capabilities.is_empty() {
        println!("host capabilities: {}", host_capabilities.join(", "));
    }

    if plan.project.source_files.is_empty() {
        return Err(LegionError::msg(report!("没有找到任何源码文件")));
    }

    if args.debug_artifacts {
        write_host_selection_spec(&plan.output_dir, &plan.project.selected_host_providers)?;
    }
    else {
        remove_host_selection_spec(&plan.output_dir)?;
    }

    let api_plan = crate::legion::api::LegionBuildPlan::from_build_plan(plan.clone());
    let compile_report =
        crate::legion::api::compile(&api_plan, crate::legion::api::CompileOptions { verbose: true, debug_artifacts: args.debug_artifacts })?;
    if !compile_report.success {
        let message = compile_report.diagnostics.first().map(|d| d.message.clone()).unwrap_or_else(|| "compile failed".into());
        return Err(LegionError::msg(message));
    }
    let report = compile_report.driver.as_ref().ok_or_else(|| LegionError::msg(report!("编译成功但缺少驱动报告")))?;
    println!("backend status: compiled");
    let target_profile = plan.project.build_target.target.to_profile(None);
    println!("host kind: {:?}", target_profile.host_kind);
    println!("publish format: {}", target_profile.artifact_policy.default_publish_format);
    if let Some(entry_symbol) = &compile_report.entry_symbol {
        println!("entry: {}", entry_symbol);
    }
    print_artifacts(&plan.output_dir, &report.artifacts);
    let manifest_source = if script::is_script_path(&plan.project.manifest_path) {
        let script_source = fs::read_to_string(&plan.project.manifest_path)
            .map_err(|e| LegionError::msg(format!("读取单脚本失败：{}: {e}", plan.project.manifest_path.display())))?;
        script::extract_embedded_manifest(&script_source, &plan.project.manifest_path)
            .map_err(|error| LegionError::from(error))?
            .ok_or_else(|| LegionError::msg(report!("单脚本 `{}` 缺少内嵌 `# ```legion` 块", plan.project.manifest_path.display())))?
    }
    else {
        fs::read_to_string(&plan.project.manifest_path)
            .map_err(|e| LegionError::msg(format!("读取项目清单失败：{}: {e}", plan.project.manifest_path.display())))?
    };
    let project_manifest = ProjectManifest::parse(&manifest_source)?;
    if let Some(plugin) = &project_manifest.build_plugin {
        unity_export::export_unity_project(&plan, &report, plugin)?;
        println!("unity export: {}", plugin.kind);
    }

    Ok(ExitCode::SUCCESS)
}

fn run_all_workspace_members(args: &BuildArgs) -> LegionResult<ExitCode> {
    let workspace = WorkspaceResolver::discover_for_project(&args.project_dir)?;
    let requested_target = args.resolved.clone();
    let members = workspace
        .legions_member_dirs()
        .into_iter()
        .filter(|project_dir| {
            let request = BuildRequest {
                project_dir: project_dir.clone(),
                resolved: requested_target.clone(),
                output_dir: args.output_dir.clone(),
                ..Default::default()
            };
            match workspace.build_plan(&request) {
                Ok(_) => true,
                Err(crate::legion::workspace::planner::PlannerError::MissingBuildTarget { .. }) => {
                    eprintln!("workspace: skipping {} (target not declared)", project_dir.display());
                    false
                }
                Err(error) => {
                    eprintln!("workspace: skipping {} (planner error: {})", project_dir.display(), error);
                    false
                }
            }
        })
        .collect::<Vec<_>>();
    if members.is_empty() {
        return Err(LegionError::msg(report!("workspace 没有可构建成员")));
    }

    println!("workspace mode: 顺序构建 {} 个成员（{}）", members.len(), crate::legion::cmds::path_for_cli_log(&workspace.root_dir));

    let mut failures = Vec::new();
    for project_dir in members {
        let member_args = BuildArgs {
            project_dir: project_dir.clone(),
            resolved: args.resolved.clone(),
            output_dir: args.output_dir.clone(),
            debug_artifacts: args.debug_artifacts,
        };
        match run_inner(&member_args) {
            Ok(code) if code == ExitCode::SUCCESS => {}
            Ok(_) => failures.push(format!("{}: exited with failure code", project_dir.display())),
            Err(error) => failures.push(format!("{}: {}", project_dir.display(), error)),
        }
    }

    if failures.is_empty() {
        println!("workspace 构建完成：全部成功");
        Ok(ExitCode::SUCCESS)
    }
    else {
        for failure in &failures {
            eprintln!("workspace build failure: {failure}");
        }
        Ok(ExitCode::FAILURE)
    }
}

/// 编译给定 `BuildPlan`，返回驱动编译报告。
pub fn compile_plan(plan: &BuildPlan, verbose: bool) -> LegionResult<nyar_emitter::DriverCompileReport> {
    if plan.project.source_files.is_empty() {
        return Err(LegionError::msg(report!("没有找到任何源码文件")));
    }

    let cache = CompilationCache::open(cache_root_for(&plan.workspace_root));
    // 缓存身份消费解析服务冻结的完整 identity：平台 + 宿主合同 + CPython ABI
    // 配置（扩展后缀、导入库）。同一平台上普通原生产物、Node 扩展与不同解释器
    // 的 CPython 扩展不是同一份产物。
    let canonical_triple = plan.project.resolved.identity();
    let manifest_files = plan
        .project
        .semantic_source_groups
        .iter()
        .map(|group| group.manifest_dir.join("legion.von"))
        .chain(std::iter::once(plan.project.manifest_path.clone()))
        .collect::<Vec<_>>();
    let ir_hash = compute_artifact_hash(
        &plan.project.source_files,
        &manifest_files,
        &canonical_triple,
        plan.project.build_target.wat,
        plan.project.build_target.runtime_async,
        &plan.enabled_features,
        Some(&crate::legion::workspace::planner::provider_set_digest(&plan.project.selected_host_providers)),
    )
    .map_err(|error| report!("{error}"))?;

    // WASI 与 Node 同为 wasm32，但宿主不同：Node 走 `env.*`，WASI 走 guest/host_contract。
    // 模板 `<% match arch %>` 因此对 WASI 使用伪架构键 `"wasi"`，避免误选 wasm32/Node 分支。
    let arch = compile_arch(plan);
    let source_groups = compile_source_snapshot(&plan.project.semantic_source_groups)?;
    if verbose {
        println!("frontend: semantic source groups={}", plan.project.semantic_source_groups.len());
    }
    let target_profile = plan.project.build_target.target.to_profile(None);
    let clr_suspend_strategy = ClrSuspendStrategy::from_runtime_async_flag(plan.project.build_target.runtime_async);
    // 产物用途由清单、宿主合同与测试模式显式声明；Wasm 容器形态从用途派生，
    // 不得反过来由容器枚举推导全 lane 的交付形态。
    let host_extension = matches!(plan.project.resolved.contract, ArtifactContract::NodeApi | ArtifactContract::Cpython(_));
    let artifact_purpose = if plan.project.is_test {
        // 测试构建：`[test]` 是唯一的 run 入口，不得要求 `[main]`
        // 或把测试回落成普通程序入口。
        if host_extension {
            return Err(LegionError::msg("宿主扩展产物不能作为测试构建：测试运行器没有宿主装载链"));
        }
        nyar::ArtifactPurpose::TestBundle
    }
    else if host_extension {
        // 宿主合同只与库交付组合：程序入口与导出面是独立事实，
        // binary + 宿主合同必须在写出前拒绝，不得改标或补造入口。
        match plan.project.artifact_kind {
            ProjectArtifactKind::Library => nyar::ArtifactPurpose::HostExtension,
            ProjectArtifactKind::Binary => {
                return Err(LegionError::msg("宿主扩展不接受程序入口：把产物声明为 library 并以导出面提供宿主 API"));
            }
        }
    }
    else {
        match plan.project.artifact_kind {
            ProjectArtifactKind::Library => nyar::ArtifactPurpose::Library,
            ProjectArtifactKind::Binary => nyar::ArtifactPurpose::Application,
        }
    };
    let wasm_package_kind = if matches!(artifact_purpose, nyar::ArtifactPurpose::TestBundle | nyar::ArtifactPurpose::Library) {
        nyar_emitter::nyar_backend_wasi::WasmPackageKind::Library
    }
    else {
        nyar_emitter::nyar_backend_wasi::WasmPackageKind::Binary
    };
    fs::create_dir_all(&plan.output_dir).map_err(|e| LegionError::msg(format!("创建输出目录失败: {}: {e}", plan.output_dir.display())))?;
    let host_bindings =
        compiler_host_provider_bindings(&plan.project.selected_host_providers).map_err(|error| LegionError::msg(report!("{error}")))?;
    let gated_out_host_providers = crate::legion::workspace::planner::compiler_gated_out_host_providers(&plan.project.gated_out_host_providers);
    // 编译计划只消费解析服务冻结的 `ResolvedTarget`：平台与合同一起进入，
    // 不在这里重新拼装或降级成单一四元组。
    let build_context = CompilerBuildContext::new(arch, plan.project.resolved.clone(), clr_suspend_strategy, wasm_package_kind)
        .with_artifact_purpose(artifact_purpose)
        .with_test_entries(plan.project.test_entries.clone())
        .with_selected_host_providers(host_bindings)
        .with_gated_out_host_providers(gated_out_host_providers);
    // 产物事务：并发序列化 → 全部产物、sidecar、evidence 与运行合同写入暂存目录
    // → 提交前完整性验证 → 受管理 generation 原子提交（可回滚）。消费者只会看到
    // 完整提交的一代；本代消失的旧受管理制品随提交退出，非受管理的用户文件不动。
    // 缓存写入失败与交付提交成功分别报告：已提交的交付不会因缓存故障被谎报失败。
    let _build_lock = acquire_build_lock(&plan.output_dir)?;
    let staging_dir = staging_dir_for(&plan.output_dir);
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir)
        .map_err(|e| LegionError::msg(format!("创建产物暂存目录失败: {}: {e}", staging_dir.display())))?;
    let outcome = (|| -> LegionResult<nyar_emitter::DriverCompileReport> {
        let artifact = match compile_source_groups_to_artifacts(
            &crate::compiler::hir::lowering::ValkyrieCompiler::default(),
            &nyar_types::compiler_input::CompilerSourceInput::application(source_groups),
            &build_context,
            &staging_dir,
            &plan.project.name,
            plan.project.build_target.wat,
            target_profile.artifact_policy.generate_runtime_config,
        ) {
            Ok(artifact) => artifact,
            Err(error) => return Err(compile_error_with_diagnostics(error, plan)),
        };
        write_compile_evidence(&staging_dir, &artifact.evidence)?;
        for managed_abi in &artifact.driver.managed_abi {
            write_managed_abi(&staging_dir, managed_abi)?;
        }
        for ovm_abi in &artifact.driver.ovm_abi {
            write_ovm_abi(&staging_dir, ovm_abi)?;
        }
        if verbose {
            println!("compiler: canonical artifact set ready");
        }

        // Write execution manifest before collecting the artifact-set so restore can run.
        // 库产物可能没有可执行 run contract（wasm 走 `callExport`），但仍必须写入
        // 导出索引，因此库构建无条件写 manifest。合法无 manifest 的交付不写它：
        // 上一代的运行合同属于受管理制品，会随本代提交退出，不再提交后单独补删。
        let is_library = plan.project.artifact_kind == ProjectArtifactKind::Library;
        if is_library || !artifact.driver.run_contracts.is_empty() {
            write_execution_manifest(&staging_dir, plan, &artifact.driver.run_contracts, &artifact.driver.library_exports, &artifact.evidence)?;
        }

        // 提交前完整性验证：报告声明的全部产物、运行合同条目与 sidecar
        // 必须在暂存集合中齐备；缺失即整组失败，不提交部分交付。
        collect_build_bundle(&staging_dir, &artifact.driver).map_err(|error| report!("产物集合不完整（暂存目录）：{error}"))?;

        commit_staged_delivery(&staging_dir, &plan.output_dir, &canonical_triple, &ir_hash)?;
        Ok(artifact.driver)
    })();
    let driver = match outcome {
        Ok(driver) => driver,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(error);
        }
    };
    // 交付已提交；缓存只是加速路径，失败单独报告为警告，不推翻已成功的交付。
    match collect_build_bundle(&plan.output_dir, &driver)
        .map_err(|error| report!("产物集合不完整：{error}"))
        .and_then(|bundle| store_cached_build(&cache, &plan.project.name, &canonical_triple, &ir_hash, &bundle).map_err(|error| report!("产物缓存写入失败：{error}")))
    {
        Ok(()) => {
            if verbose {
                println!("cache: stored (artifact-set)");
            }
        }
        Err(error) => {
            eprintln!("warning: 交付已提交，但产物缓存写入失败（下次构建将重新编译）: {error}");
        }
    }

    Ok(driver)
}

/// 暂存目录：输出目录内的隐藏子目录，按进程与序号命名，避免并发构建互相覆盖。
fn staging_dir_for(output_dir: &Path) -> PathBuf {
    static STAGING_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let seq = STAGING_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    output_dir.join(format!(".nyar-staging-{}-{seq}", std::process::id()))
}

/// 受管理制品的 generation 身份文件：记录最近一次成功提交的受管理文件清单。
///
/// 消费者只应把这份清单列出的文件当作上一代交付；清单之外的文件是用户内容，
/// 提交事务不清空它们。文件本身是元数据，不在受管理清单内。
pub(crate) const DELIVERY_MANIFEST_FILE: &str = ".nyar-delivery.json";

/// 并发提交事务的锁文件：只在一代提交期间存在，不是产物。
pub(crate) const BUILD_LOCK_FILE: &str = ".nyar-build.lock";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct DeliveryManifest {
    /// 身份 schema 版本。
    schema_version: u32,
    /// 本代身份（目标三元组 + IR 摘要）。
    generation: String,
    /// 受管理产物文件名（相对输出目录）。
    files: Vec<String>,
}

/// 并发序列化：同一输出目录同时只允许一个提交事务。
#[derive(Debug)]
struct BuildLock {
    path: PathBuf,
}

impl Drop for BuildLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_build_lock(output_dir: &Path) -> LegionResult<BuildLock> {
    let path = output_dir.join(BUILD_LOCK_FILE);
    match fs::File::create_new(&path) {
        Ok(_) => Ok(BuildLock { path }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(LegionError::msg(format!("输出目录正被另一个构建占用，等待其完成后重试: {}", output_dir.display())))
        }
        Err(error) => Err(LegionError::msg(format!("创建构建锁失败 {}: {error}", path.display()))),
    }
}

fn read_delivery_manifest(output_dir: &Path) -> DeliveryManifest {
    // 读不到的清单按"无受管理上一代"处理：首次提交或被中断的旧格式交付。
    fs::read(output_dir.join(DELIVERY_MANIFEST_FILE)).ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// 提交整组暂存产物为新一代受管理交付，可回滚。
///
/// 顺序：预校验暂存集合 → 把将被覆盖与退出的上一代受管理文件移入备份目录 →
/// 移入本代文件 → 写 generation 清单 → 清理备份。任何一步失败都把备份移回，
/// 输出目录恢复上一代完整状态；消费者不会看到半代交付，也不会因本代缺失而
/// 残留旧受管理制品（运行合同退出同样由删除集覆盖，不再有提交后补删）。
fn commit_staged_delivery(staging_dir: &Path, output_dir: &Path, triple: &str, ir_hash: &str) -> LegionResult<()> {
    // 预校验：只允许普通文件，先把本代集合固定下来。
    let mut staged = Vec::new();
    for entry in fs::read_dir(staging_dir).map_err(|e| LegionError::msg(format!("枚举暂存目录失败: {}: {e}", staging_dir.display())))? {
        let entry = entry.map_err(|e| LegionError::msg(format!("读取暂存目录项失败: {e}")))?;
        let file_type = entry.file_type().map_err(|e| LegionError::msg(format!("暂存目录项类型不可读: {}: {e}", entry.path().display())))?;
        if !file_type.is_file() {
            return Err(LegionError::msg(format!("产物暂存目录不应包含子目录: {}", entry.path().display())));
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            return Err(LegionError::msg(format!("暂存产物文件名不是合法 UTF-8: {}", entry.path().display())));
        };
        if name == DELIVERY_MANIFEST_FILE || name.starts_with(".nyar-") {
            return Err(LegionError::msg(format!("暂存目录不应包含事务元数据: {name}")));
        }
        staged.push((entry.path(), name));
    }
    let staged_names: Vec<&str> = staged.iter().map(|(_, name)| name.as_str()).collect();

    let previous = read_delivery_manifest(output_dir);
    static BACKUP_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let backup_seq = BACKUP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let backup_dir = output_dir.join(format!(".nyar-backup-{}-{backup_seq}", std::process::id()));
    fs::create_dir_all(&backup_dir).map_err(|e| LegionError::msg(format!("创建提交备份目录失败: {}: {e}", backup_dir.display())))?;

    // 回滚记录：moved_aside = (备份位置, 原位置)；moved_in = 本代已移入的路径。
    let mut moved_aside: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut moved_in: Vec<PathBuf> = Vec::new();
    let rollback = |moved_aside: &[(PathBuf, PathBuf)], moved_in: &[PathBuf]| {
        // 先撤本代已移入的文件，再从备份恢复上一代文件。
        for to in moved_in.iter().rev() {
            let _ = fs::remove_file(to);
        }
        for (from, to) in moved_aside.iter().rev() {
            let _ = fs::rename(from, to);
        }
        let _ = fs::remove_dir_all(&backup_dir);
    };

    // 1) 让位：本代将覆盖的文件 + 上一代受管理但本代不再存在的退出项。
    let retire: Vec<&str> = previous.files.iter().map(String::as_str).filter(|name| !staged_names.contains(name)).collect();
    let replace = staged_names.clone();
    for name in replace.iter().chain(retire.iter()) {
        let existing = output_dir.join(name);
        if existing.is_file() {
            let to = backup_dir.join(name);
            if let Err(error) = fs::rename(&existing, &to) {
                rollback(&moved_aside, &moved_in);
                return Err(LegionError::msg(format!("移出旧受管理文件失败 {}: {error}", existing.display())));
            }
            moved_aside.push((to, existing));
        }
    }

    // 2) 移入本代。
    for (from, name) in &staged {
        let to = output_dir.join(name);
        if let Err(error) = fs::rename(from, &to) {
            rollback(&moved_aside, &moved_in);
            return Err(LegionError::msg(format!("提交产物失败: {} -> {}: {error}", from.display(), to.display())));
        }
        moved_in.push(to);
    }

    // 3) 写 generation 清单（受管理集合的最终事实）。
    let manifest = DeliveryManifest {
        schema_version: 0,
        generation: format!("{triple}-{ir_hash}"),
        files: staged.iter().map(|(_, name)| name.clone()).chain(std::iter::once(DELIVERY_MANIFEST_FILE.to_string())).collect(),
    };
    // 清单自身作为受管理文件在下一代表达；此处直接写入输出目录。
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        rollback(&moved_aside, &moved_in);
        LegionError::msg(format!("序列化 generation 清单失败: {error}"))
    });
    match manifest_bytes {
        Ok(bytes) => {
            if let Err(error) = fs::write(output_dir.join(DELIVERY_MANIFEST_FILE), bytes) {
                rollback(&moved_aside, &moved_in);
                return Err(LegionError::msg(format!("写入 generation 清单失败: {error}")));
            }
        }
        Err(error) => return Err(error),
    }

    // 4) 清理：备份目录与暂存目录。
    for (backup, _) in &moved_aside {
        let _ = fs::remove_file(backup);
    }
    let _ = fs::remove_dir_all(&backup_dir);
    fs::remove_dir_all(staging_dir).map_err(|e| LegionError::msg(format!("清理暂存目录失败: {}: {e}", staging_dir.display())))?;
    Ok(())
}

#[cfg(test)]
mod artifact_transaction_tests {
    use super::*;

    /// 提交整组暂存产物：新文件进入交付、同名旧文件被替换、generation 清单落盘。
    #[test]
    fn commit_moves_whole_staged_set_and_replaces_old_delivery() {
        let root = tempfile::tempdir().expect("temp dir");
        let output = root.path().join("dist");
        fs::create_dir_all(&output).expect("output dir");
        fs::write(output.join("stale-only.txt"), b"user file").expect("write user file");
        fs::write(output.join("app.wasm"), b"old bytes").expect("write old artifact");

        let staging = staging_dir_for(&output);
        fs::create_dir_all(&staging).expect("staging dir");
        fs::write(staging.join("app.wasm"), b"new bytes").expect("write new artifact");
        fs::write(staging.join("compile-evidence.txt"), b"evidence").expect("write sidecar");

        commit_staged_delivery(&staging, &output, "wasm32-unknown-unknown", "ir1").expect("提交整组暂存产物");

        assert_eq!(fs::read(output.join("app.wasm")).expect("read artifact"), b"new bytes", "同名旧产物必须被整组替换");
        assert!(output.join("compile-evidence.txt").is_file(), "sidecar 与产物同批提交");
        assert!(output.join("stale-only.txt").is_file(), "非受管理的用户文件不被事务清空");
        assert!(!staging.exists(), "提交成功后暂存目录必须清空");
        let manifest = read_delivery_manifest(&output);
        assert_eq!(manifest.generation, "wasm32-unknown-unknown-ir1");
        assert!(manifest.files.iter().any(|name| name == "app.wasm"), "{manifest:?}");
        assert!(manifest.files.iter().any(|name| name == DELIVERY_MANIFEST_FILE), "清单自身受管理: {manifest:?}");
    }

    /// 上一代受管理而本代消失的制品（如运行合同）随提交退出；用户文件保留。
    #[test]
    fn commit_retires_previous_managed_files() {
        let root = tempfile::tempdir().expect("temp dir");
        let output = root.path().join("dist");
        fs::create_dir_all(&output).expect("output dir");
        // 手工布置上一代：manifest 声明 app.wasm 与 run-contracts.txt 受管理。
        fs::write(output.join("app.wasm"), b"g1").expect("write g1");
        fs::write(output.join("run-contracts.txt"), b"g1 contracts").expect("write g1 contracts");
        fs::write(output.join("user-notes.md"), b"keep me").expect("write user file");
        let previous = DeliveryManifest {
            schema_version: 0,
            generation: "g1".to_string(),
            files: vec!["app.wasm".to_string(), "run-contracts.txt".to_string(), DELIVERY_MANIFEST_FILE.to_string()],
        };
        fs::write(output.join(DELIVERY_MANIFEST_FILE), serde_json::to_vec(&previous).expect("serialize")).expect("write manifest");

        let staging = staging_dir_for(&output);
        fs::create_dir_all(&staging).expect("staging dir");
        fs::write(staging.join("app.wasm"), b"g2").expect("write g2");
        // 本代没有 run-contracts.txt：上一代受管理副本必须退出。

        commit_staged_delivery(&staging, &output, "wasm32-unknown-unknown", "ir2").expect("提交第二代交付");

        assert_eq!(fs::read(output.join("app.wasm")).expect("read g2"), b"g2");
        assert!(!output.join("run-contracts.txt").exists(), "本代消失的受管理运行合同必须随提交退出");
        assert!(output.join("user-notes.md").is_file(), "用户文件不清空");
    }

    /// 提交中途失败必须回滚到上一代完整状态，不留半代交付。
    #[test]
    fn commit_rolls_back_to_previous_generation_on_failure() {
        let root = tempfile::tempdir().expect("temp dir");
        let output = root.path().join("dist");
        fs::create_dir_all(&output).expect("output dir");
        fs::write(output.join("app.wasm"), b"g1").expect("write g1");
        // 注入失败：本代第二个目标位置是一个目录，rename 无法覆盖它。
        fs::create_dir_all(output.join("run-contracts.txt")).expect("blocking directory");
        let previous = DeliveryManifest {
            schema_version: 0,
            generation: "g1".to_string(),
            files: vec!["app.wasm".to_string(), DELIVERY_MANIFEST_FILE.to_string()],
        };
        fs::write(output.join(DELIVERY_MANIFEST_FILE), serde_json::to_vec(&previous).expect("serialize")).expect("write manifest");

        let staging = staging_dir_for(&output);
        fs::create_dir_all(&staging).expect("staging dir");
        fs::write(staging.join("app.wasm"), b"g2").expect("write g2");
        fs::write(staging.join("run-contracts.txt"), b"g2 contracts").expect("write g2 contracts");

        commit_staged_delivery(&staging, &output, "wasm32-unknown-unknown", "ir2").expect_err("目标被占用必须整组失败");

        assert_eq!(fs::read(output.join("app.wasm")).expect("read after rollback"), b"g1", "回滚后上一代产物完整恢复");
        assert!(output.join(DELIVERY_MANIFEST_FILE).is_file(), "generation 清单回滚保留");
        assert!(read_delivery_manifest(&output).generation == "g1", "清单仍指向上第一代");
    }

    /// 暂存集合出现子目录说明装配违约：提交拒绝，不部分移动。
    #[test]
    fn commit_rejects_unexpected_subdirectory() {
        let root = tempfile::tempdir().expect("temp dir");
        let output = root.path().join("dist");
        fs::create_dir_all(&output).expect("output dir");
        let staging = staging_dir_for(&output);
        fs::create_dir_all(staging.join("nested")).expect("staging with subdirectory");

        let error = commit_staged_delivery(&staging, &output, "t", "h").expect_err("子目录必须被拒绝");
        assert!(error.to_string().contains("子目录"), "{error}");
    }

    /// 同一输出目录的提交事务串行：锁未释放前第二个构建拒绝进入。
    #[test]
    fn build_lock_serializes_concurrent_commits() {
        let root = tempfile::tempdir().expect("temp dir");
        let output = root.path();
        let first = acquire_build_lock(output).expect("first lock");
        let error = acquire_build_lock(output).expect_err("second lock must be rejected while held");
        assert!(error.to_string().contains("占用"), "{error}");
        drop(first);
        acquire_build_lock(output).expect("lock reacquired after release");
    }

    /// 并发构建的暂存目录互不覆盖。
    #[test]
    fn staging_directories_are_unique() {
        let root = tempfile::tempdir().expect("temp dir");
        let first = staging_dir_for(root.path());
        let second = staging_dir_for(root.path());
        assert_ne!(first, second);
        assert!(first.file_name().expect("name").to_string_lossy().starts_with(".nyar-staging-"));
    }
}

/// 编译失败时取出本编译单元暂存的全部可恢复诊断，渲染成多诊断报告。
///
/// 没有暂存诊断（例如规划、后端或装配阶段的失败）保持原有单错误文本不变；
/// 有暂存诊断时以完整报告作为错误消息：同一编译单元的全部独立错误一次列出，
/// 按 `(文件, span)` 稳定排序、同一根因归并、超出上限给汇总。
/// 源码原文按诊断的 `source.id`（相对组 manifest 目录的逻辑路径）懒读磁盘，
/// 与 `compile_source_snapshot` 同样做 BOM 归一化，保证 span → 行列映射一致。
fn compile_error_with_diagnostics(error: crate::compiler::emitter::pipeline::PipelineError, plan: &BuildPlan) -> LegionError {
    let pending = crate::compiler::diagnostics::pending::take_pending_diagnostics();
    if pending.is_empty() {
        return LegionError::from(error);
    }
    // 最终失败事实不得被多诊断报告吞掉：暂存汇已含同 code 条目说明它已被记录，
    // 否则在报告末尾补一行最终失败（rustc 在 aborting 行后同型给出最终失败）。
    let fatal_code = match &error {
        crate::compiler::emitter::pipeline::PipelineError::Hir(hir) => Some(hir.code()),
        _ => None,
    };
    let fatal_text = error.to_string();
    let represented = match fatal_code {
        Some(code) => pending.diagnostics.iter().any(|diagnostic| diagnostic.row.code == code),
        None => pending.diagnostics.iter().any(|diagnostic| diagnostic.message == fatal_text),
    };
    let options = crate::compiler::diagnostics::report::DiagnosticRenderOptions::from_environment();
    let mut report = crate::compiler::diagnostics::report::render_compile_diagnostics(
        &pending,
        &|id| {
            plan.project.semantic_source_groups.iter().find_map(|group| {
                let content = std::fs::read_to_string(group.manifest_dir.join(id)).ok()?;
                Some(content.strip_prefix('\u{FEFF}').map(str::to_string).unwrap_or(content))
            })
        },
        &options,
    );
    if !represented {
        report.push_str(&format!("error: {fatal_text}\n"));
    }
    if report.trim().is_empty() { LegionError::from(error) } else { LegionError::msg(report.trim_end().to_string()) }
}

/// 编译期架构键：WASI 与 Node 同为 wasm32，但宿主不同，因此使用伪架构键 `"wasi"`。
pub(crate) fn compile_arch(plan: &BuildPlan) -> &'static str {
    crate::legion::workspace::planner::compile_arch_key(&plan.project.build_target.target)
}

/// 将 managed ABI 合同落盘为确定性 pretty JSON。
///
/// 每个物理产物一份 sidecar，文件名由产物名唯一派生（`app.jar` →
/// `app.jar.managed-abi.json`）；多分区构建不得互相覆盖。
fn write_managed_abi(output_dir: &Path, contract: &nyar_emitter::ManagedAbiContract) -> LegionResult<()> {
    let output_path = output_dir.join(nyar_emitter::managed_abi_sidecar_file(&contract.artifact));
    let content = serde_json::to_string_pretty(contract)
        .map_err(|error| LegionError::msg(format!("序列化 managed ABI 契约失败 {}: {error}", output_path.display())))?;
    fs::create_dir_all(output_dir).map_err(|e| LegionError::msg(format!("创建输出目录失败: {}: {e}", output_dir.display())))?;
    fs::write(&output_path, format!("{content}\n"))
        .map_err(|e| LegionError::msg(format!("写入 managed ABI 契约失败 {}: {e}", output_path.display())))?;
    Ok(())
}

/// 将 OVM ABI sidecar 落盘为确定性 pretty JSON：合同 + `.odin` 产物整体哈希。
///
/// 每个物理产物一份 sidecar，文件名由产物名唯一派生（`app.odin` →
/// `app.odin.ovm-abi.json`）；宿主读取后必须先用实际 `.odin` 字节重算哈希，
/// 再按合同解释，不一致 fail-closed。
fn write_ovm_abi(output_dir: &Path, sidecar: &nyar_emitter::OvmAbiSidecar) -> LegionResult<()> {
    let output_path = output_dir.join(nyar_emitter::ovm_abi_sidecar_file(&sidecar.artifact));
    let content = serde_json::to_string_pretty(sidecar)
        .map_err(|error| LegionError::msg(format!("序列化 OVM ABI sidecar 失败 {}: {error}", output_path.display())))?;
    fs::create_dir_all(output_dir).map_err(|e| LegionError::msg(format!("创建输出目录失败: {}: {e}", output_dir.display())))?;
    fs::write(&output_path, format!("{content}\n"))
        .map_err(|e| LegionError::msg(format!("写入 OVM ABI sidecar 失败 {}: {e}", output_path.display())))?;
    Ok(())
}

fn write_compile_evidence(output_dir: &Path, evidence: &crate::compiler::emitter::pipeline::CompilerCompileEvidence) -> LegionResult<()> {
    let output_path = output_dir.join("compile-evidence.txt");
    let content = crate::legion::write_von_indented(evidence).map_err(|error| {
        labeled_report_with_context(error, format!("序列化 compile evidence 失败 {}", output_path.display()), "VON 序列化失败位置")
    })?;
    fs::create_dir_all(output_dir).map_err(|e| LegionError::msg(format!("创建输出目录失败: {}: {e}", output_dir.display())))?;
    fs::write(&output_path, content).map_err(|e| LegionError::msg(format!("写入 compile evidence 失败: {}: {e}", output_path.display())))?;
    Ok(())
}

pub(super) fn print_artifacts(output_dir: &Path, artifacts: &ArtifactSet) {
    for artifact in &artifacts.artifacts {
        let artifact_path = output_dir.join(&artifact.path);
        if !artifact_path.is_file() {
            println!("artifact: missing {}", artifact.path);
            continue;
        }
        println!("artifact: {}", artifact_path.display());
    }
}

fn write_execution_manifest(
    output_dir: &Path,
    plan: &crate::legion::workspace::planner::BuildPlan,
    specs: &[DriverRunContract],
    library_exports: &[nyar_emitter::DriverLibraryExport],
    evidence: &crate::compiler::emitter::pipeline::CompilerCompileEvidence,
) -> LegionResult<()> {
    let contracts = specs
        .iter()
        .map(|spec| RunContract {
            logical_entry: spec.logical_entry.clone(),
            physical_entry: spec.physical_entry.clone(),
            invocation: spec.invocation.clone(),
            validate: spec.validate.clone(),
        })
        .collect::<Vec<_>>();
    let exports = library_exports
        .iter()
        .map(|export| crate::legion::cmds::run::LibraryExportIndexEntry {
            logical_name: export.logical_name.clone(),
            physical_artifact: export.physical_artifact.clone(),
            invocation: export.invocation.clone(),
            abi_digest: export.abi_digest.clone(),
        })
        .collect::<Vec<_>>();
    ExecutionManifest::from_build_plan(plan, output_dir, &contracts, &exports, evidence)?.write_to_output_dir(output_dir)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct HostSelectionEntry {
    contract: String,
    provider: String,
    source_file: String,
    line: usize,
    required_capabilities: Vec<String>,
    foreign_bindings: Vec<String>,
}

fn write_host_selection_spec(output_dir: &Path, providers: &[crate::legion::workspace::planner::PlannedHostProvider]) -> LegionResult<()> {
    let output_path = output_dir.join("host-selection.txt");
    let entries: Vec<HostSelectionEntry> = providers
        .iter()
        .map(|item| HostSelectionEntry {
            contract: item.contract.clone(),
            provider: item.symbol.clone(),
            source_file: item.source_file.display().to_string(),
            line: item.line,
            required_capabilities: item.required_capabilities.clone(),
            foreign_bindings: item.foreign_bindings.clone(),
        })
        .collect();
    let content = write_von_indented(&entries).map_err(|error| {
        labeled_report_with_context(error, format!("序列化 host 选择结果失败 {}", output_path.display()), "VON 序列化失败位置")
    })?;
    fs::create_dir_all(output_dir).map_err(|e| LegionError::msg(format!("创建输出目录失败 {}: {e}", output_dir.display())))?;
    fs::write(&output_path, content).map_err(|e| LegionError::msg(format!("写入 host 选择结果失败 {}: {e}", output_path.display())))?;
    Ok(())
}

fn remove_host_selection_spec(output_dir: &Path) -> LegionResult<()> {
    let output_path = output_dir.join("host-selection.txt");
    if output_path.exists() {
        fs::remove_file(&output_path).map_err(|e| LegionError::msg(format!("删除 host 选择结果失败 {}: {e}", output_path.display())))?;
    }
    Ok(())
}
