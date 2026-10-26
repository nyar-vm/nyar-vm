//! `legion fmt`：格式化 `.v` / `.vx` / `.von` / `.awsl`。
use crate::legion::cmds::error::{LegionError, LegionResult};
use nyar_format::formatter::{FormatCliOptions, format_options_from_cli, normalize_extensions, report_format_cli_with, run_format_cli_with};
use std::process::ExitCode;

/// `legion fmt` 参数。
#[derive(Debug, Clone)]
pub struct FmtArgs {
    /// 要格式化的文件或目录（默认当前目录）。
    pub paths: Vec<std::path::PathBuf>,
    /// 仅检查是否已格式化，不写盘；有差异时退出码非 0。
    pub check: bool,
    /// 忽略 `.editorconfig`，使用 CLI 指定的缩进/行宽。
    pub no_editorconfig: bool,
    /// 打印每个被改写的文件。
    pub verbose: bool,
    /// 有文件因解析错误被跳过时以非 0 退出。
    pub fail_on_skip: bool,
    /// 缩进宽度（空格数）；仅在与 `--no-editorconfig` 联用时生效。
    pub indent_width: Option<usize>,
    /// 折行行宽；仅在与 `--no-editorconfig` 联用时生效。
    pub max_width: Option<usize>,
    /// 仅处理指定扩展名（可重复，如 `--ext awsl --ext v`）。
    pub extensions: Vec<String>,
}

impl FmtArgs {
    fn to_cli_options(&self) -> FormatCliOptions {
        let extensions = if self.extensions.is_empty() { None } else { Some(normalize_extensions(&self.extensions)) };
        FormatCliOptions {
            paths: self.paths.clone(),
            check: self.check,
            use_editorconfig: !self.no_editorconfig,
            verbose: self.verbose,
            fail_on_skip: self.fail_on_skip,
            format: format_options_from_cli(self.indent_width, self.max_width),
            extensions,
        }
    }
}

/// 执行 `legion fmt`。
pub fn run(args: &FmtArgs) -> LegionResult<ExitCode> {
    let options = args.to_cli_options();
    let report = run_format_cli_with(&options).map_err(|error| report!("{error}"))?;
    if report_format_cli_with(&report, &options) { Ok(ExitCode::FAILURE) } else { Ok(ExitCode::SUCCESS) }
}
