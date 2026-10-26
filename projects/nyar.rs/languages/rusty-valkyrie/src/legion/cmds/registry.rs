use crate::legion::cmds::error::{LegionError, LegionResult};
use nyar_package_manager::PackageManager;
use std::process::ExitCode;

/// `legion registry` arguments.
#[derive(Debug, Clone)]
pub struct RegistryArgs {
    /// registry 子命令。
    pub command: RegistryCommand,
}

/// Registry source subcommands.
#[derive(Debug, Clone)]
pub enum RegistryCommand {
    /// List configured registry endpoints.
    List,
    /// Add or update a registry endpoint.
    Add(RegistryAddArgs),
    /// Remove a custom registry endpoint (built-ins reset to defaults).
    Remove(RegistryRemoveArgs),
    /// Show details for one registry.
    Info(RegistryInfoArgs),
}

/// `legion registry add` arguments.
#[derive(Debug, Clone)]
pub struct RegistryAddArgs {
    /// Registry name (`npm`, `jsr`, `nuget`, `maven`, `conda`, `valhalla`).
    pub name: String,
    /// Registry HTTP endpoint.
    pub endpoint: String,
}

/// `legion registry remove` arguments.
#[derive(Debug, Clone)]
pub struct RegistryRemoveArgs {
    /// Registry name.
    pub name: String,
}

/// `legion registry info` arguments.
#[derive(Debug, Clone)]
pub struct RegistryInfoArgs {
    /// Registry name.
    pub name: String,
}

/// Run `legion registry`.
pub fn run(args: &RegistryArgs) -> LegionResult<ExitCode> {
    let mut legion = PackageManager::open(".", crate::legion::LEGION_PROJECT_LAYOUT)
        .map_err(|e| LegionError::msg(format!("{e}: {}", "打开包管理器上下文失败")))?;
    match &args.command {
        RegistryCommand::List => {
            let sources = legion.registry_list();
            if sources.is_empty() {
                println!("没有配置注册表源");
            }
            else {
                for (name, endpoint) in sources {
                    println!("{name}\t{endpoint}");
                }
            }
            println!("# config: {}", legion.registry_sources_path().display());
            Ok(ExitCode::SUCCESS)
        }
        RegistryCommand::Add(add) => {
            legion.registry_add(&add.name, &add.endpoint)?;
            println!("已设置 {} -> {}", add.name.to_ascii_lowercase(), add.endpoint.trim_end_matches('/'));
            Ok(ExitCode::SUCCESS)
        }
        RegistryCommand::Remove(remove) => {
            if legion.registry_remove(&remove.name)? {
                println!("已重置/移除 {}", remove.name.to_ascii_lowercase());
                Ok(ExitCode::SUCCESS)
            }
            else {
                println!("未找到注册表 {}", remove.name);
                Ok(ExitCode::FAILURE)
            }
        }
        RegistryCommand::Info(info) => {
            let (name, endpoint, is_default) = legion.registry_info(&info.name)?;
            println!("name:     {name}");
            println!("endpoint: {endpoint}");
            println!("default:  {is_default}");
            println!("config:   {}", legion.registry_sources_path().display());
            Ok(ExitCode::SUCCESS)
        }
    }
}
