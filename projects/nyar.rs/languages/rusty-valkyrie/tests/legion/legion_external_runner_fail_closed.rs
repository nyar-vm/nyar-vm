//! `legion test` 外部 runner fail-closed 分类测试。
//!
//! 锁定事实：运行时未安装、产物格式不匹配、执行超时、退出码非零必须各自返回
//! 结构化 `ExternalRunFailure` 分类与可诊断文本，禁止静默跳过或当作通过。
//! 用主机自带 shell 构造可控的退出码与耗时场景，Linux/macOS 走 `sh`，Windows 走 `cmd.exe`。

use std::{path::PathBuf, time::Duration};

use rusty_valkyrie::legion::cmds::test_engine::{ExternalRunFailure, execute_external_runner};

fn artifact_with_extension(ext: &str) -> PathBuf {
    PathBuf::from(format!("fixture/test-artifact.{ext}"))
}

fn cmd_args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// 主机 shell 程序与参数：测试不允许依赖只在单一平台存在的可执行文件。
fn shell(command_text: &str) -> (String, Vec<String>) {
    if cfg!(windows) {
        ("cmd.exe".to_string(), cmd_args(&["/C", command_text]))
    }
    else {
        ("sh".to_string(), cmd_args(&["-c", command_text]))
    }
}

#[test]
fn missing_runtime_command_is_classified_as_runtime_not_installed() {
    let outcome = execute_external_runner(
        "definitely-not-installed-runner-xyz",
        &cmd_args(&["fixture.jar"]),
        &artifact_with_extension("jar"),
        ".jar",
        Duration::from_secs(5),
        false,
    );
    assert!(!outcome.success);
    assert!(!outcome.is_compile_error);
    assert_eq!(outcome.failure, Some(ExternalRunFailure::RuntimeNotInstalled));
    let error = outcome.error.expect("运行时未安装必须携带可诊断原因");
    assert!(error.contains("definitely-not-installed-runner-xyz"), "{error}");
}

#[test]
fn artifact_format_mismatch_is_rejected_before_spawn() {
    let (program, args) = shell("exit 0");
    let outcome = execute_external_runner(
        &program,
        &args,
        &artifact_with_extension("wasm"),
        ".jar",
        Duration::from_secs(5),
        false,
    );
    assert!(!outcome.success);
    assert_eq!(outcome.failure, Some(ExternalRunFailure::ArtifactFormatMismatch));
    let error = outcome.error.expect("格式不匹配必须携带可诊断原因");
    assert!(error.contains("wasm") && error.contains("jar"), "{error}");
}

#[test]
fn non_zero_exit_is_classified_with_exit_code() {
    let (program, args) = shell("exit 3");
    let outcome = execute_external_runner(
        &program,
        &args,
        &artifact_with_extension("jar"),
        ".jar",
        Duration::from_secs(10),
        false,
    );
    assert!(!outcome.success);
    assert_eq!(outcome.failure, Some(ExternalRunFailure::NonZeroExit));
    let error = outcome.error.expect("非零退出必须携带退出码诊断");
    assert!(error.contains('3'), "{error}");
}

#[test]
fn slow_runner_is_terminated_and_classified_as_timeout() {
    // 约 30 秒的睡眠；500ms 超时必须终止进程并分类为 ExecutionTimeout。
    let (program, args) = shell(if cfg!(windows) { "ping -n 30 127.0.0.1" } else { "sleep 30" });
    let outcome = execute_external_runner(
        &program,
        &args,
        &artifact_with_extension("jar"),
        ".jar",
        Duration::from_millis(500),
        false,
    );
    assert!(!outcome.success);
    assert_eq!(outcome.failure, Some(ExternalRunFailure::ExecutionTimeout));
    let error = outcome.error.expect("超时必须携带可诊断原因");
    assert!(error.contains("500"), "{error}");
}

#[test]
fn clean_exit_zero_run_passes() {
    let (program, args) = shell("exit 0");
    let outcome = execute_external_runner(
        &program,
        &args,
        &artifact_with_extension("jar"),
        ".jar",
        Duration::from_secs(10),
        false,
    );
    assert!(outcome.success, "{:?}", outcome.error);
    assert_eq!(outcome.failure, None);
}
