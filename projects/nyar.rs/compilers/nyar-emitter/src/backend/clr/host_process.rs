//! 托管宿主子进程的静默启动合同。
//!
//! CLR 产物在宿主侧硬故障或未处理异常时会触发 Windows 错误报告对话框，
//! 批量测试与 legion 运行会连续打断桌面。这里集中提供两个事实：
//!
//! - [`configure_quiet_host_command`]：为 `dotnet`/`java` 子进程关闭控制台窗口、
//!   断开 stdin，并抑制崩溃转储与遥测；
//! - [`suppress_host_error_dialogs`]：进程级 `SetErrorMode`，子进程继承该掩码，
//!   从而不再弹出硬错误对话框。
//!
//! 两个函数只影响进程呈现，不改变产物字节或执行语义。

use std::process::{Command, Stdio};

/// 把托管宿主子进程配置为静默运行。
///
/// 调用方仍负责捕获或消费 stdout/stderr（例如 [`Command::output`]）；
/// 本函数只关闭控制台窗口、断开 stdin 并设置宿主静默环境。
pub fn configure_quiet_host_command(command: &mut Command) -> &mut Command {
    command.stdin(Stdio::null());
    command.env("DOTNET_NOLOGO", "1");
    command.env("DOTNET_CLI_TELEMETRY_OPTOUT", "1");
    command.env("DOTNET_DbgEnableMiniDump", "0");
    command.env("DOTNET_EnableCrashReport", "0");
    command.env("DOTNET_DefaultDiagnosticPortSuspend", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// 抑制本进程及子进程的 Windows 硬错误对话框。
///
/// `SetErrorMode` 的错误模式位会被子进程继承，因此调用一次即可覆盖后续所有
/// `dotnet`/`java` 启动。非 Windows 平台是空操作。
pub fn suppress_host_error_dialogs() {
    #[cfg(windows)]
    {
        const SEM_FAILCRITICALERRORS: u32 = 0x0001;
        const SEM_NOGPFAULTERRORBOX: u32 = 0x0002;
        const SEM_NOOPENFILEERRORBOX: u32 = 0x8000;
        unsafe extern "system" {
            fn SetErrorMode(mode: u32) -> u32;
        }
        // SAFETY: `SetErrorMode` 只修改本进程的错误模式位，没有内存安全前置条件。
        unsafe {
            SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
        }
    }
}
