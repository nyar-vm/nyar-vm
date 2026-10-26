use crate::legion::cmds::error::{LegionError, LegionResult};
use nyar_package_manager::PackageManager;
use std::process::ExitCode;

/// `legion login` arguments.
#[derive(Debug, Clone)]
pub struct LoginArgs {
    /// Registry name (`npm`, `jsr`, …). Defaults to `publishConfig.registry` or `npm`.
    pub registry: Option<String>,
    /// Auth token. When omitted, discovers from official CLI stores (`~/.npmrc`, Deno config, …).
    pub token: Option<String>,
}

/// `legion logout` arguments.
#[derive(Debug, Clone)]
pub struct LogoutArgs {
    /// Registry name. Defaults to `publishConfig.registry` or `npm`.
    pub registry: Option<String>,
}

/// `legion whoami` arguments.
#[derive(Debug, Clone)]
pub struct WhoamiArgs {
    /// Registry name. Defaults to `publishConfig.registry` or `npm`.
    pub registry: Option<String>,
}

/// Run `legion login`.
pub fn run_login(args: &LoginArgs) -> LegionResult<ExitCode> {
    let mut legion = PackageManager::open(".", crate::legion::LEGION_PROJECT_LAYOUT)
        .map_err(|e| LegionError::msg(format!("{e}: {}", "打开包管理器上下文失败")))?;
    let registry = args.registry.as_deref();
    let result = legion.login(registry, args.token.as_deref()).map_err(|e| LegionError::msg(format!("{e}: {}", "登录失败")))?;

    if result.verify.valid {
        let username = result.verify.username.unwrap_or_else(|| "unknown".to_string());
        println!("已登录 {} 为 {}（{}）", registry_label(registry, &legion), username, result.credential_source);
        if result.stored_in_auth_von {
            println!("凭据已写入 ~/.valkyrie/auth.von");
        }
        else if nyar_package_manager::is_external_registry(&legion.login_registry_name(registry)) {
            println!("凭据来自官方 CLI 配置，未写入 auth.von");
        }
        if let Some(target) = result.synced_to {
            println!("已同步到官方 CLI 存储：{target}");
        }
        Ok(ExitCode::SUCCESS)
    }
    else {
        eprintln!("登录失败: {}", result.verify.error_message.unwrap_or_else(|| "令牌验证失败".to_string()));
        Ok(ExitCode::FAILURE)
    }
}

/// Run `legion logout`.
pub fn run_logout(args: &LogoutArgs) -> LegionResult<ExitCode> {
    let mut legion = PackageManager::open(".", crate::legion::LEGION_PROJECT_LAYOUT)
        .map_err(|e| LegionError::msg(format!("{e}: {}", "打开包管理器上下文失败")))?;
    let registry = args.registry.as_deref();
    if legion.logout(registry)? {
        println!("已登出 {}", registry_label(registry, &legion));
    }
    else {
        println!("未找到 {} 的凭证", registry_label(registry, &legion));
    }
    Ok(ExitCode::SUCCESS)
}

/// Run `legion whoami`.
pub fn run_whoami(args: &WhoamiArgs) -> LegionResult<ExitCode> {
    let legion = PackageManager::open(".", crate::legion::LEGION_PROJECT_LAYOUT)
        .map_err(|e| LegionError::msg(format!("{e}: {}", "打开包管理器上下文失败")))?;
    let registry = args.registry.as_deref();
    let result = legion.whoami(registry)?;
    if result.valid {
        println!("{}", result.username.unwrap_or_else(|| "unknown".to_string()));
        Ok(ExitCode::SUCCESS)
    }
    else {
        eprintln!("{}", result.error_message.unwrap_or_else(|| "未登录".to_string()));
        Ok(ExitCode::FAILURE)
    }
}

fn registry_label(registry: Option<&str>, legion: &PackageManager) -> String {
    registry.map(str::trim).filter(|value| !value.is_empty()).map(str::to_string).unwrap_or_else(|| legion.login_registry_name(None))
}
