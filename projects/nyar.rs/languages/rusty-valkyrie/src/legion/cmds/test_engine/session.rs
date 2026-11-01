//! 测试构建与执行会话。
use crate::legion::{
    cmds::error::{LegionError, LegionResult},
    workspace::{
        manifest::RunnerBinding,
        planner::{BuildRequest, WorkspaceResolver},
    },
};
use nyar::{CanonicalTarget, RunnerFamily};
use nyar_runner::{RuntimeContract as InterpreterRuntimeContract, RuntimeFamily as InterpreterRuntimeFamily};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use crate::legion::cmds::{
    build::compile_plan,
    report::TestResultEntry,
    run::{ExecutionManifest, RunContract, select_artifact},
};

use super::{
    discover::{DiscoveredFunction, discover_project_tests},
    targets::parse_target_label,
};

/// 外部 runner 的结构化失败分类。
///
/// 外部 runner 必须 fail-closed：运行时缺失、产物格式不符、执行超时、退出码非零
/// 都要归入可诊断类别，禁止静默跳过或当作通过。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalRunFailure {
    /// runner 命令未安装或不在 `PATH` 上，无法启动。
    RuntimeNotInstalled,
    /// 产物扩展名与目标 profile 声明的主产物格式不一致，拒绝启动 runner。
    ArtifactFormatMismatch,
    /// 执行超过时限，进程已被终止。
    ExecutionTimeout,
    /// 进程以非零退出码结束，或退出码为零但 `stderr` 非空。
    NonZeroExit,
}

impl ExternalRunFailure {
    /// 测试输出与报告使用的诊断标签。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeNotInstalled => "runtime-not-installed",
            Self::ArtifactFormatMismatch => "artifact-format-mismatch",
            Self::ExecutionTimeout => "execution-timeout",
            Self::NonZeroExit => "non-zero-exit",
        }
    }
}

/// 单次测试执行结果。
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// 测试是否通过。
    pub success: bool,
    /// 失败是否发生在编译/会话阶段（而非运行期）。
    pub is_compile_error: bool,
    /// 运行期失败的结构化分类；通过或编译期失败为 `None`。
    pub failure: Option<ExternalRunFailure>,
    /// 失败时的结构化错误文本；通过时为 `None`。
    pub error: Option<String>,
}

/// 外部 target 测试会话：产物落盘后复用。
pub struct ExternalTestSession {
    /// 会话对应的 target 标签。
    pub target: String,
    /// 测试产物输出目录。
    pub output_dir: PathBuf,
    /// Compiler 产出的逐入口运行合同。
    pub run_contracts: Vec<RunContract>,
}

/// 编译 target 测试会话。
pub fn compile_external_test_session(
    workspace: &WorkspaceResolver,
    project_dir: &Path,
    target_label: &str,
) -> std::result::Result<ExternalTestSession, String> {
    let target = parse_target_label(target_label).map_err(|e| e.to_string())?;
    let output_dir = project_dir.join(".cache").join("test").join(target_label);
    let request = BuildRequest { project_dir: project_dir.to_path_buf(), target, output_dir: Some(output_dir.clone()), ..Default::default() };
    let (plan, _) = workspace.build_test_plan(&request).map_err(|e| e.to_string())?;
    compile_plan(&plan, false).map_err(|e| e.to_string())?;
    let manifest = ExecutionManifest::read_from_output_dir(&output_dir)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "compiler did not produce an execution manifest".to_string())?;
    if !manifest.is_fresh_for_plan(&plan).map_err(|e| e.to_string())? {
        return Err("compiler produced a stale execution manifest".into());
    }
    if manifest.run_contracts.is_empty() {
        return Err("compiler produced no runtime execution contracts".into());
    }
    Ok(ExternalTestSession { target: target_label.to_string(), output_dir, run_contracts: manifest.run_contracts })
}

/// 在外部会话中执行单个测试函数。
pub fn run_external_test_in_session(
    workspace: &WorkspaceResolver,
    session: &ExternalTestSession,
    function_name: &str,
    cli_runners: &[String],
    verbose: bool,
) -> RunOutcome {
    let Some(artifact) = resolve_external_test_artifact_path(session, function_name)
    else {
        return RunOutcome {
            success: false,
            is_compile_error: true,
            failure: None,
            error: Some(format!("未找到测试 `{function_name}` 对应的产物。{}", describe_runnable_artifacts(session))),
        };
    };

    let Ok(canonical) = parse_target_label(&session.target)
    else {
        return RunOutcome { success: false, is_compile_error: true, failure: None, error: Some(format!("未知目标 {}", session.target)) };
    };

    // 运行器家族必须由 CanonicalTarget 的 profile 推导；按标签字符串猜测会把
    // `wasm32-node-unknown-wasm` 这类完整三元组误判为 Ovm，进而静默换成错误的宿主命令。
    let runner_family = canonical.to_profile(None).runner_family();

    // OVM 家族由 legion 内置的 odin-vm 解释器进程内执行，不存在独立 `ovm` 指令，
    // 也不解析外部 `odin-vm` 二进制路径或 runner 覆盖。
    if runner_family == RunnerFamily::Ovm {
        return run_builtin_ovm_test(&artifact, function_name, verbose);
    }

    let template = match resolve_runner_template(workspace, &canonical, runner_family, cli_runners, function_name) {
        Ok(t) => t,
        Err(error) => return RunOutcome { success: false, is_compile_error: true, failure: None, error: Some(error.to_string()) },
    };

    let placeholders = build_placeholders(&session.output_dir, &artifact, runner_family, function_name);
    let command = expand_placeholders(&template.command, &placeholders);
    let args: Vec<String> = template.args.iter().map(|a| expand_placeholders(a, &placeholders)).collect();

    // 格式门以目标 profile 的主产物扩展名为唯一事实源，不按标签字符串猜测每个 lane 的产物形态。
    let expected_extension = canonical.to_profile(None).artifact_policy.primary_extension;
    execute_external_runner(&command, &args, &artifact, &expected_extension, EXTERNAL_RUNNER_TIMEOUT, verbose)
}

/// 外部 runner 默认执行时限。
pub const EXTERNAL_RUNNER_TIMEOUT: Duration = Duration::from_secs(60);

/// fail-closed 执行外部 runner：格式门 → 启动分类 → 超时轮询 → 退出码分类。
///
/// 四类运行期失败各自返回 `ExternalRunFailure` 结构化分类与可诊断文本，
/// 绝不把「未安装/超时/格式不符」静默折叠成通过或无差别失败。
pub fn execute_external_runner(
    command: &str,
    args: &[String],
    artifact: &Path,
    expected_extension: &str,
    timeout: Duration,
    verbose: bool,
) -> RunOutcome {
    let expected = expected_extension.trim_start_matches('.').to_ascii_lowercase();
    if !expected.is_empty() {
        let actual = artifact.extension().map(|ext| ext.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if actual != expected {
            return RunOutcome {
                success: false,
                is_compile_error: false,
                failure: Some(ExternalRunFailure::ArtifactFormatMismatch),
                error: Some(format!(
                    "产物格式不匹配: `{}` 扩展名 `{actual}` 与目标主产物格式 `.{expected}` 不一致，拒绝启动 runner",
                    artifact.display()
                )),
            };
        }
    }

    let mut child = match Command::new(command).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            let reason = if error.kind() == std::io::ErrorKind::NotFound { "运行时未安装或不在 PATH" } else { "运行时无法启动" };
            return RunOutcome {
                success: false,
                is_compile_error: false,
                failure: Some(ExternalRunFailure::RuntimeNotInstalled),
                error: Some(format!("{reason}: `{command}` — {error}")),
            };
        }
    };

    // 独立线程排空管道，避免子进程因写满管道缓冲而阻塞、被误判为超时。
    let stdout_pipe = child.stdout.take().expect("piped stdout");
    let stderr_pipe = child.stderr.take().expect("piped stderr");
    let stdout_reader = std::thread::spawn(move || drain_pipe(stdout_pipe));
    let stderr_reader = std::thread::spawn(move || drain_pipe(stderr_pipe));

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return RunOutcome {
                        success: false,
                        is_compile_error: false,
                        failure: Some(ExternalRunFailure::ExecutionTimeout),
                        error: Some(format!("执行超时: `{command}` 未在 {} ms 内退出，已终止进程", timeout.as_millis())),
                    };
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                return RunOutcome {
                    success: false,
                    is_compile_error: false,
                    failure: Some(ExternalRunFailure::RuntimeNotInstalled),
                    error: Some(format!("等待 runner `{command}` 失败: {error}")),
                };
            }
        }
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();

    if status.success() {
        if !stderr.trim().is_empty() {
            return RunOutcome { success: false, is_compile_error: false, failure: Some(ExternalRunFailure::NonZeroExit), error: Some(stderr) };
        }
        if verbose && !stdout.trim().is_empty() {
            println!("      stdout: {}", stdout.trim_end());
        }
        return RunOutcome { success: true, is_compile_error: false, failure: None, error: None };
    }

    let code = status.code().map(|code| code.to_string()).unwrap_or_else(|| "signal".to_string());
    let mut detail = stderr;
    if detail.trim().is_empty() {
        detail = stdout;
    }
    RunOutcome {
        success: false,
        is_compile_error: false,
        failure: Some(ExternalRunFailure::NonZeroExit),
        error: Some(format!("runner `{command}` 退出码 {code}: {}", detail.trim())),
    }
}

fn drain_pipe(mut pipe: impl Read) -> String {
    let mut buffer = Vec::new();
    if pipe.read_to_end(&mut buffer).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buffer).to_string()
}

/// 用 legion 内置 OVM 运行时进程内执行单个测试入口。
///
/// 加载与执行失败一律 fail-closed；`vm.run` 返回 `Ok` 即记为通过，
/// runner 不猜测、不特判 VM 语义（例如无 handler 的 `raise` 挂起行为）。
fn run_builtin_ovm_test(artifact: &Path, function_name: &str, verbose: bool) -> RunOutcome {
    let bytes = match std::fs::read(artifact) {
        Ok(bytes) => bytes,
        Err(error) => {
            return RunOutcome {
                success: false,
                is_compile_error: true,
                failure: None,
                error: Some(format!("读取 OVM 产物 `{}` 失败: {error}", artifact.display())),
            };
        }
    };
    let mut vm = odin_vm::OdinVm::new();
    let loaded = match vm.load(&bytes) {
        Ok(loaded) => loaded,
        Err(error) => {
            return RunOutcome {
                success: false,
                is_compile_error: true,
                failure: None,
                error: Some(format!("OVM 加载产物 `{}` 失败: {error}", artifact.display())),
            };
        }
    };
    match vm.run(&loaded, function_name, Vec::new()) {
        Ok(value) => {
            if verbose {
                println!("      ovm: {value:?}");
            }
            RunOutcome { success: true, is_compile_error: false, failure: None, error: None }
        }
        Err(error) => {
            RunOutcome {
                success: false,
                is_compile_error: false,
                failure: Some(ExternalRunFailure::NonZeroExit),
                error: Some(format!("OVM 执行 `{function_name}` 失败: {error}")),
            }
        }
    }
}

/// 为项目在多 target 上运行全部测试。
pub fn run_tests_for_project(
    workspace: &WorkspaceResolver,
    project_dir: &Path,
    filter: Option<&str>,
    targets: &[String],
    cli_runners: &[String],
    verbose: bool,
) -> (usize, usize, usize, Vec<TestResultEntry>) {
    let test_dir = project_dir.join("test");
    if !test_dir.exists() {
        println!("  无 test/ 目录，跳过");
        return (0, 0, 0, Vec::new());
    }

    let functions = discover_project_tests(project_dir);
    if functions.is_empty() {
        println!("  未发现 [test] 标注的测试函数");
        return (0, 0, 0, Vec::new());
    }

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut total_skipped = 0usize;
    let mut results = Vec::new();

    for target in targets {
        println!("  --- {target} ---");
        let session = compile_external_test_session(workspace, project_dir, target);

        for tf in &functions {
            if filter.is_some_and(|f| !tf.name.to_ascii_lowercase().contains(&f.to_ascii_lowercase())) {
                if verbose {
                    println!("    - {} ... 已跳过（过滤）", tf.name);
                }
                total_skipped += 1;
                results.push(TestResultEntry {
                    name: tf.name.clone(),
                    status: "skip".into(),
                    error: Some("被过滤器排除".into()),
                    target: target.clone(),
                });
                continue;
            }

            if tf.is_benchmark {
                if verbose {
                    println!("    - {} ... 已跳过（[benchmark]）", tf.name);
                }
                total_skipped += 1;
                results.push(TestResultEntry {
                    name: tf.name.clone(),
                    status: "skip".into(),
                    error: Some("[benchmark] 标注".into()),
                    target: target.clone(),
                });
                continue;
            }

            if tf.compile_only {
                if verbose {
                    println!("    - {} ... 已跳过（compile_only）", tf.name);
                }
                total_skipped += 1;
                results.push(TestResultEntry {
                    name: tf.name.clone(),
                    status: "skip".into(),
                    error: Some("compile_only 夹具（见 legion-language 编译测试）".into()),
                    target: target.clone(),
                });
                continue;
            }

            if !tf.is_test {
                continue;
            }

            print!("    {} ... ", tf.name);

            let outcome = match &session {
                Ok(session) => run_external_test_in_session(workspace, session, &tf.name, cli_runners, verbose),
                Err(error) => RunOutcome { success: false, is_compile_error: true, failure: None, error: Some(error.clone()) },
            };

            if outcome.success {
                println!("ok");
                total_passed += 1;
                results.push(TestResultEntry { name: tf.name.clone(), status: "pass".into(), error: None, target: target.clone() });
            }
            else if outcome.is_compile_error {
                println!("COMPILE ERROR");
                if let Some(error) = &outcome.error {
                    println!("      {error}");
                }
                total_failed += 1;
                results.push(TestResultEntry {
                    name: tf.name.clone(),
                    status: "compile_error".into(),
                    error: outcome.error,
                    target: target.clone(),
                });
            }
            else {
                match outcome.failure {
                    Some(failure) => println!("FAILED [{}]", failure.as_str()),
                    None => println!("FAILED"),
                }
                if let Some(error) = &outcome.error {
                    println!("      {error}");
                }
                total_failed += 1;
                results.push(TestResultEntry { name: tf.name.clone(), status: "fail".into(), error: outcome.error, target: target.clone() });
            }
        }
    }

    (total_passed, total_failed, total_skipped, results)
}

/// 为项目在多 target 上运行基准测试。
pub fn bench_project(
    workspace: &WorkspaceResolver,
    project_dir: &Path,
    project_name: &str,
    runs: usize,
    targets: &[String],
    verbose: bool,
) -> Vec<crate::legion::cmds::report::BenchResultRow> {
    let functions: Vec<DiscoveredFunction> = discover_project_tests(project_dir).into_iter().filter(|f| f.is_benchmark).collect();
    if functions.is_empty() {
        println!("  未发现 [benchmark] 标注的函数");
        return Vec::new();
    }

    let mut results = Vec::new();
    for target in targets {
        for tf in &functions {
            let mut compile_times = Vec::new();
            let mut runtime_times = Vec::new();

            for _ in 0..runs {
                let compile_start = Instant::now();
                let session_result = compile_external_test_session(workspace, project_dir, target);
                let compile_ms = compile_start.elapsed().as_secs_f64() * 1000.0;

                let Ok(session) = session_result
                else {
                    continue;
                };

                let runtime_start = Instant::now();
                let outcome = run_external_test_in_session(workspace, &session, &tf.name, &[], verbose);
                let runtime_ms = runtime_start.elapsed().as_secs_f64() * 1000.0;

                if !outcome.success && verbose {
                    if let Some(error) = &outcome.error {
                        println!("      bench {} 失败: {error}", tf.name);
                    }
                }

                compile_times.push(compile_ms);
                runtime_times.push(runtime_ms);
            }

            if !compile_times.is_empty() && !runtime_times.is_empty() {
                let compile_avg = compile_times.iter().sum::<f64>() / compile_times.len() as f64;
                let runtime_avg = runtime_times.iter().sum::<f64>() / runtime_times.len() as f64;
                results.push(crate::legion::cmds::report::BenchResultRow {
                    project: project_name.to_string(),
                    test: tf.name.clone(),
                    target: target.clone(),
                    compile_ms: compile_avg,
                    runtime_ms: runtime_avg,
                });
            }
        }
    }
    results
}

/// 只消费当前 Compiler 给出的精确测试运行合同。
pub fn resolve_external_test_artifact_path(session: &ExternalTestSession, function_name: &str) -> Option<PathBuf> {
    let mut contracts = session.run_contracts.iter().filter(|contract| contract.logical_entry == function_name);
    let contract = contracts.next()?;
    if contracts.next().is_some() {
        return None;
    }
    select_artifact(&session.output_dir, std::iter::once(contract.physical_entry.as_str()), None).ok().map(|(artifact, _)| artifact)
}

fn describe_runnable_artifacts(session: &ExternalTestSession) -> String {
    let names =
        session.run_contracts.iter().map(|contract| format!("{} → {}", contract.logical_entry, contract.physical_entry)).collect::<Vec<_>>();
    format!("Compiler 运行合同：{}", names.join(", "))
}

#[derive(Debug, Clone)]
struct RunnerTemplate {
    command: String,
    args: Vec<String>,
}

fn resolve_runner_template(
    workspace: &WorkspaceResolver,
    canonical_target: &CanonicalTarget,
    runner_target: RunnerFamily,
    cli_runner_overrides: &[String],
    function_name: &str,
) -> LegionResult<RunnerTemplate> {
    let default = default_runner_template(runner_target, function_name);

    if let Some(command) = parse_runner_overrides(cli_runner_overrides)?.remove(&runner_target) {
        return Ok(RunnerTemplate { command, args: default.args });
    }

    if let Some(workspace_manifest) = &workspace.workspace_manifest {
        if let Some(binding) = workspace_manifest.runner.iter().find(|binding| runner_binding_matches(binding, runner_target, canonical_target))
        {
            return Ok(RunnerTemplate { command: binding.command.clone(), args: binding.args.clone() });
        }
    }

    if let Ok(command) = std::env::var(format!("LEGION_RUNNER_{}", runner_target.as_str().to_ascii_uppercase())) {
        if !command.trim().is_empty() {
            return Ok(RunnerTemplate { command, args: default.args });
        }
    }

    Ok(default)
}

fn runner_binding_matches(binding: &RunnerBinding, runner_target: RunnerFamily, canonical_target: &CanonicalTarget) -> bool {
    binding.target.matches(runner_target, canonical_target)
}

fn parse_runner_overrides(values: &[String]) -> LegionResult<BTreeMap<RunnerFamily, String>> {
    let mut overrides = BTreeMap::new();
    for item in values {
        let Some((target, command)) = item.split_once('=')
        else {
            return Err(LegionError::msg(report!("invalid runner override '{item}': expected target=command")));
        };
        let family = target.trim().parse::<RunnerFamily>().map_err(|error| report!("invalid runner override '{item}': {error}"))?;
        overrides.insert(family, command.trim().to_string());
    }
    Ok(overrides)
}

fn default_runner_template(target: RunnerFamily, function_name: &str) -> RunnerTemplate {
    let contract = InterpreterRuntimeContract { logical_entry: Some(function_name), physical_entry: Some(function_name), wasi_p3: false };
    let family = match target {
        RunnerFamily::Clr => InterpreterRuntimeFamily::Clr,
        RunnerFamily::Jvm => InterpreterRuntimeFamily::Jvm,
        RunnerFamily::Node => InterpreterRuntimeFamily::Node,
        RunnerFamily::Windows => InterpreterRuntimeFamily::Windows,
        RunnerFamily::Wasi => InterpreterRuntimeFamily::Wasi,
        RunnerFamily::Ovm => InterpreterRuntimeFamily::Ovm,
    };
    let template = family.default_template(Some(contract));
    RunnerTemplate { command: template.command, args: template.args }
}

fn build_placeholders(output_dir: &Path, artifact: &Path, runner_target: RunnerFamily, function_name: &str) -> BTreeMap<&'static str, String> {
    let mut values = BTreeMap::new();
    let artifact_text = strip_verbatim_prefix(&artifact.to_string_lossy()).to_string();
    values.insert("artifact", artifact_text.clone());
    let classpath = if artifact.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("jar")) {
        artifact_text.clone()
    }
    else {
        strip_verbatim_prefix(&output_dir.to_string_lossy()).to_string()
    };
    values.insert("classpath", classpath);
    let entry = if runner_target == RunnerFamily::Jvm { function_name.to_string() } else { function_name.to_string() };
    values.insert("entry", entry);
    values
}

fn strip_verbatim_prefix(path: &str) -> &str {
    path.strip_prefix(r"\\?\").unwrap_or(path)
}

fn expand_placeholders(template: &str, placeholders: &BTreeMap<&'static str, String>) -> String {
    placeholders.iter().fold(template.to_string(), |current, (key, value)| current.replace(&format!("{{{key}}}"), value))
}
