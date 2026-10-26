use crate::legion::cmds::error::{LegionError, LegionResult};
use nyar_package_manager::PackageManager;
use std::process::ExitCode;

/// `legion search` arguments.
#[derive(Debug, Clone)]
pub struct SearchArgs {
    /// Search query.
    pub query: String,
    /// Registry name.
    pub registry: String,
}

/// Run `legion search`.
pub fn run(args: &SearchArgs) -> LegionResult<ExitCode> {
    let legion = PackageManager::open(".", crate::legion::LEGION_PROJECT_LAYOUT)
        .map_err(|e| LegionError::msg(format!("{e}: {}", "打开包管理器上下文失败")))?;
    let packages = legion.search(&args.query, &args.registry).map_err(|e| LegionError::msg(format!("{e}: {}", "搜索失败")))?;
    if packages.is_empty() {
        println!("未找到匹配包");
        return Ok(ExitCode::SUCCESS);
    }
    for package in packages {
        println!("{}@{}  {}", package.name, package.version, package.description);
    }
    Ok(ExitCode::SUCCESS)
}

/// `legion info` arguments.
#[derive(Debug, Clone)]
pub struct InfoArgs {
    /// Package name.
    pub package: String,
    /// Registry name.
    pub registry: String,
}

/// Run `legion info`.
pub fn run_info(args: &InfoArgs) -> LegionResult<ExitCode> {
    let legion = PackageManager::open(".", crate::legion::LEGION_PROJECT_LAYOUT)?;
    let info = legion.get_package_info(&args.package, &args.registry)?;
    println!("name:        {}", info.name);
    println!("version:     {}", info.version);
    println!("registry:    {}", info.registry);
    println!("description: {}", info.description);
    Ok(ExitCode::SUCCESS)
}
