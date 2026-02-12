use nyar_vm::vm::platform::NyarPlatform;
use nyar_types::NyarError;

/// 原生平台实现，直接调用 Rust 标准库
#[derive(Debug)]
pub struct NativePlatform;

impl NyarPlatform for NativePlatform {
    fn stdout_write(&self, msg: &str) {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| print!("{}", msg));
        } else {
            print!("{}", msg);
        }
    }

    fn stderr_write(&self, msg: &str) {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| eprint!("{}", msg));
        } else {
            eprint!("{}", msg);
        }
    }

    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn sleep_ms(&self, ms: u64) {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| std::thread::sleep(std::time::Duration::from_millis(ms)));
        } else {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
}
