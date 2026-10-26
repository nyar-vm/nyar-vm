use crate::legion::cmds::error::{LegionError, LegionResult};
use nyar_package_manager::PackageManager;
use std::{path::PathBuf, process::ExitCode};

/// `legion install` arguments.
#[derive(Debug, Clone)]
pub struct InstallArgs {
    /// Optional package name. When omitted, installs manifest dependencies.
    pub package: Option<String>,
    /// Package version (default: latest).
    pub version: String,
    /// Registry name.
    pub registry: String,
    /// Project directory.
    pub project_dir: PathBuf,
    /// Fail if lockfile would change.
    pub frozen_lockfile: bool,
    /// Install only from local cache/vendors; fail immediately when a package is missing.
    pub offline: bool,
    /// Include devDependencies.
    pub dev: bool,
}

/// Run `legion install`.
pub fn run(args: &InstallArgs) -> LegionResult<ExitCode> {
    let mut pm = PackageManager::open(&args.project_dir, crate::legion::LEGION_PROJECT_LAYOUT)
        .map_err(|e| LegionError::msg(format!("{e}: {}", "打开包管理器上下文失败")))?;
    pm.frozen_lockfile = args.frozen_lockfile;
    pm.offline = args.offline;

    if let Some(package) = &args.package {
        let info =
            pm.install_one(package, &args.version, &args.registry, true).map_err(|e| LegionError::msg(format!("{e}: {}", "安装包失败")))?;
        println!("已安装 {}@{} ({})", info.name, info.version, info.registry);
    }
    else {
        let installed = pm.install_dependencies(args.dev, &args.registry).map_err(|e| LegionError::msg(format!("{e}: {}", "安装依赖失败")))?;
        println!("已安装 {} 个依赖", installed.len());
        for info in installed {
            println!("  - {}@{}", info.name, info.version);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `legion add` arguments.
#[derive(Debug, Clone)]
pub struct AddArgs {
    /// Package name.
    pub package: String,
    /// Version constraint.
    pub version: String,
    /// Registry name.
    pub registry: String,
    /// Project directory.
    pub project_dir: PathBuf,
}

/// Run `legion add`.
pub fn run_add(args: &AddArgs) -> LegionResult<ExitCode> {
    let mut pm = PackageManager::open(&args.project_dir, crate::legion::LEGION_PROJECT_LAYOUT)?;
    let info = pm.add_dependency(&args.package, &args.version, &args.registry)?;
    println!("已添加依赖 {}@{}", info.name, info.version);
    Ok(ExitCode::SUCCESS)
}

/// `legion remove` arguments.
#[derive(Debug, Clone)]
pub struct RemoveArgs {
    /// Package name.
    pub package: String,
    /// Project directory.
    pub project_dir: PathBuf,
}

/// Run `legion remove`.
pub fn run_remove(args: &RemoveArgs) -> LegionResult<ExitCode> {
    let mut pm = PackageManager::open(&args.project_dir, crate::legion::LEGION_PROJECT_LAYOUT)?;
    let info = pm.remove_dependency(&args.package)?;
    println!("已移除依赖 {}", info.name);
    Ok(ExitCode::SUCCESS)
}

/// `legion update` arguments.
#[derive(Debug, Clone)]
pub struct UpdateArgs {
    /// Optional package name. When omitted, updates all locked packages.
    pub package: Option<String>,
    /// Version to update to.
    pub version: String,
    /// Registry name.
    pub registry: String,
    /// Project directory.
    pub project_dir: PathBuf,
}

/// Run `legion update`.
pub fn run_update(args: &UpdateArgs) -> LegionResult<ExitCode> {
    let mut pm = PackageManager::open(&args.project_dir, crate::legion::LEGION_PROJECT_LAYOUT)?;
    if let Some(package) = &args.package {
        let info = pm.update_one(package, &args.version, &args.registry)?;
        println!("已更新 {}@{}", info.name, info.version);
    }
    else {
        let updated = pm.update_all(&args.registry)?;
        println!("已更新 {} 个依赖", updated.len());
        for info in updated {
            println!("  - {}@{}", info.name, info.version);
        }
    }
    Ok(ExitCode::SUCCESS)
}
