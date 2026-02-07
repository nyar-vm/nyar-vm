use std::sync::Arc;
use crate::vm::platform::NyarPlatform;

/// 原生平台实现，直接调用 Rust 标准库
pub struct NativePlatform;

impl NyarPlatform for NativePlatform {
    fn stdout_write(&self, msg: &str) {
        print!("{}", msg);
    }

    fn stdin_read_line(&self) -> String {
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        input.trim_end().to_string()
    }

    fn proc_exit(&self, code: i32) -> ! {
        std::process::exit(code);
    }

    fn clock_now(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
    }

    fn get_env(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn fs_read_to_string(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn fs_write(&self, path: &str, content: &str) -> Result<(), String> {
        std::fs::write(path, content).map_err(|e| e.to_string())
    }

    fn fs_exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    fn fs_remove_file(&self, path: &str) -> Result<(), String> {
        std::fs::remove_file(path).map_err(|e| e.to_string())
    }

    fn random_f64(&self) -> f64 {
        rand::random()
    }

    fn sleep(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

#[cfg(feature = "wasi")]
/// WASI 平台实现，调用 WASI 系统接口
pub struct WasiPlatform;

#[cfg(feature = "wasi")]
impl NyarPlatform for WasiPlatform {
    fn stdout_write(&self, msg: &str) {
        // 使用 wasi 库进行 fd_write
        let iov = [wasi::Ciovec { buf: msg.as_ptr(), buf_len: msg.len() }];
        unsafe {
            let _ = wasi::fd_write(1, &iov);
        }
    }

    fn stdin_read_line(&self) -> String {
        let mut res = String::new();
        let mut buf = [0u8; 1];
        let iov = [wasi::Iovec { buf: buf.as_mut_ptr(), buf_len: 1 }];
        unsafe {
            loop {
                let mut nread = 0;
                if wasi::fd_read(0, &iov, &mut nread) != 0 || nread == 0 {
                    break;
                }
                if buf[0] == b'\n' {
                    break;
                }
                res.push(buf[0] as char);
            }
        }
        res.trim_end().to_string()
    }

    fn proc_exit(&self, code: i32) -> ! {
        unsafe {
            wasi::proc_exit(code as u32);
        }
    }

    fn clock_now(&self) -> f64 {
        unsafe {
            let mut time = 0;
            // CLOCKID_REALTIME = 0
            if wasi::clock_time_get(0, 1000, &mut time) == 0 {
                return time as f64 / 1_000_000_000.0;
            }
        }
        0.0
    }

    fn get_env(&self, _key: &str) -> Option<String> {
        // TODO: WASI environ_get
        None
    }

    fn fs_read_to_string(&self, _path: &str) -> Result<String, String> {
        // TODO: WASI fd_read with path opening
        Err("WasiPlatform: fs_read_to_string not fully implemented".to_string())
    }

    fn fs_write(&self, _path: &str, _content: &str) -> Result<(), String> {
        // TODO: WASI fd_write with path opening
        Err("WasiPlatform: fs_write not fully implemented".to_string())
    }

    fn fs_exists(&self, _path: &str) -> bool {
        // TODO: WASI path_filestat_get
        false
    }

    fn fs_remove_file(&self, _path: &str) -> Result<(), String> {
        // TODO: WASI path_unlink_file
        Err("WasiPlatform: fs_remove_file not fully implemented".to_string())
    }

    fn random_f64(&self) -> f64 {
        let mut buf = [0u8; 8];
        unsafe {
            if wasi::random_get(&mut buf) == 0 {
                return u64::from_le_bytes(buf) as f64 / u64::MAX as f64;
            }
        }
        0.0
    }

    fn sleep(&self, ms: u64) {
        // WASI sleep using poll_oneoff
        let clock = wasi::SubscriptionClock {
            id: 0, // CLOCKID_REALTIME
            timeout: ms * 1_000_000,
            precision: 1_000_000,
            flags: 0,
        };
        let subscription = wasi::Subscription {
            userdata: 0,
            u: wasi::SubscriptionU {
                tag: wasi::EVENTTYPE_CLOCK,
                u: wasi::SubscriptionUU { clock },
            },
        };
        let mut event = wasi::Event {
            userdata: 0,
            error: 0,
            type_: 0,
            fd_readwrite: wasi::EventFdReadwrite { nbytes: 0, flags: 0 },
        };
        let mut nevents = 0;
        unsafe {
            let _ = wasi::poll_oneoff(&subscription, &mut event, 1, &mut nevents);
        }
    }
}
