//! `legion test` 外部 runner fail-closed 分类测试。
//!
//! 锁定事实：运行时未安装、产物格式不匹配、执行超时、退出码非零必须各自返回
//! 结构化 `ExternalRunFailure` 分类与可诊断文本，禁止静默跳过或当作通过。
//! 主机为 Windows，用 `cmd.exe` 构造可控的退出码与耗时场景。

use std::{path::PathBuf, time::Duration};

use rusty_valkyrie::legion::cmds::test_engine::{ExternalRunFailure, execute_external_runner};

fn artifact_with_extension(ext: &str) -> PathBuf {
    PathBuf::from(format!("fixture/test-artifact.{ext}"))
}

fn cmd_args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
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
    let outcome = execute_external_runner(
        "cmd.exe",
        &cmd_args(&["/C", "exit 0"]),
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
    let outcome = execute_external_runner(
        "cmd.exe",
        &cmd_args(&["/C", "exit 3"]),
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
    // `ping -n 30` 约耗时 30 秒；500ms 超时必须终止进程并分类为 ExecutionTimeout。
    let outcome = execute_external_runner(
        "cmd.exe",
        &cmd_args(&["/C", "ping", "-n", "30", "127.0.0.1"]),
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
    let outcome = execute_external_runner(
        "cmd.exe",
        &cmd_args(&["/C", "exit 0"]),
        &artifact_with_extension("jar"),
        ".jar",
        Duration::from_secs(10),
        false,
    );
    assert!(outcome.success, "{:?}", outcome.error);
    assert_eq!(outcome.failure, None);
}
