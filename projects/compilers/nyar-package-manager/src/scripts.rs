use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::{PackageManagerError, Result};

/// 生命周期脚本执行结果。
#[derive(Debug, Clone)]
pub struct ScriptResult {
    /// 脚本名称（如 `prePublish`）。
    pub name: String,
    /// 进程是否以零退出码结束。
    pub success: bool,
    /// 子进程退出码；信号终止时为 `None`。
    pub exit_code: Option<i32>,
    /// 标准输出内容。
    pub stdout: String,
    /// 标准错误内容。
    pub stderr: String,
}

/// 在包根目录下执行 manifest 中声明的脚本命令。
pub struct ScriptRunner<'a> {
    package_path: &'a Path,
}

impl<'a> ScriptRunner<'a> {
    /// 绑定到 `package_path` 作为脚本工作目录。
    pub fn new(package_path: &'a Path) -> Self {
        Self { package_path }
    }

    /// 在包目录执行 `command`，不注入额外环境变量。
    pub fn run(&self, name: &str, command: &str) -> Result<ScriptResult> {
        self.run_with_env(name, command, &[])
    }

    /// Run a lifecycle script with additional process environment variables.
    ///
    /// Product CLIs may inject import/search paths (e.g. `PYTHONPATH`, `NODE_PATH`)
    /// without teaching this crate language-specific layout rules.
    pub fn run_with_env(&self, name: &str, command: &str, env: &[(&str, &str)]) -> Result<ScriptResult> {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        }
        else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };
        cmd.current_dir(self.package_path).stdout(Stdio::piped()).stderr(Stdio::piped());
        for (key, value) in env {
            cmd.env(key, value);
        }
        let output = cmd.output().map_err(|error| PackageManagerError::message(format!("failed to run script `{name}`: {error}")))?;

        Ok(ScriptResult {
            name: name.to_string(),
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
