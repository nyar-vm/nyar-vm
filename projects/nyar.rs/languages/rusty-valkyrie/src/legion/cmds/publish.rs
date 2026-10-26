use crate::legion::cmds::error::{LegionError, LegionResult};
use nyar_package_manager::{PackageManager, PublishOptions, VersionBump};
use std::{path::PathBuf, process::ExitCode};

/// `legion publish` arguments.
#[derive(Debug, Clone)]
pub struct PublishArgs {
    /// Project directory (default: current directory).
    pub project_dir: PathBuf,
    /// Target registry name. When omitted, every registry entry in `legion.von` `publish` is published.
    pub registry: Option<String>,
    /// Publish tag (`latest`, `beta`, …).
    pub tag: Option<String>,
    /// Access level (`public` / `restricted`).
    pub access: Option<String>,
    /// Version bump (`patch` / `minor` / `major`).
    pub bump: Option<String>,
    /// Validate and pack without uploading.
    pub dry_run: bool,
    /// Skip unclean git working-tree check.
    pub skip_git_check: bool,
    /// Do not create a git tag after success.
    pub no_git_tag: bool,
    /// Publish every workspace member.
    pub workspace: bool,
    /// Explicit auth token (otherwise uses stored / env token).
    pub token: Option<String>,
}

/// Run `legion publish`.
pub fn run(args: &PublishArgs) -> LegionResult<ExitCode> {
    let mut legion = PackageManager::open(&args.project_dir, crate::legion::LEGION_PROJECT_LAYOUT)
        .map_err(|e| LegionError::msg(format!("{e}: {}", "打开包管理器上下文失败")))?;
    let bump = args
        .bump
        .as_deref()
        .map(|value| VersionBump::parse(value).ok_or_else(|| LegionError::msg(report!("无效的 --bump 值: {value}（期望 patch/minor/major）"))))
        .transpose()?;

    let options = PublishOptions {
        registry_name: args.registry.clone().unwrap_or_default(),
        tag: args.tag.clone(),
        access: args.access.clone(),
        bump,
        dry_run: args.dry_run,
        skip_git_check: args.skip_git_check,
        create_git_tag: !args.no_git_tag && !args.dry_run,
        auth_token: args.token.clone(),
        ..PublishOptions::default()
    };

    let result = legion.publish(options, args.workspace).map_err(|e| LegionError::msg(format!("{e}: {}", "发布失败")))?;
    if result.success {
        if result.dry_run {
            println!(
                "dry-run 成功: {}@{} ({} bytes, {} files, {})",
                result.package_name,
                result.version,
                result.size.unwrap_or(0),
                result.file_count.unwrap_or(0),
                result.sha256.as_deref().unwrap_or("-")
            );
        }
        else {
            println!("发布成功: {}@{}", result.package_name, result.version);
            if let Some(url) = &result.published_url {
                println!("URL: {url}");
            }
        }
        Ok(ExitCode::SUCCESS)
    }
    else if result.official_tool_required {
        println!("需要官方工具发布 {}@{}", result.package_name, result.version);
        println!("{}", result.message);
        if let Some(url) = &result.published_url {
            println!("registry: {url}");
        }
        Ok(ExitCode::FAILURE)
    }
    else {
        eprintln!("发布失败: {}", result.message);
        Ok(ExitCode::FAILURE)
    }
}
