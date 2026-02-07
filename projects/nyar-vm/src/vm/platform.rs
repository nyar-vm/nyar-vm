/// Nyar 平台抽象接口，符合 WASI 规范的语义
pub trait NyarPlatform: Send + Sync {
    /// 向标准输出写入数据 (对应 WASI fd_write)
    fn stdout_write(&self, msg: &str);

    /// 从标准输入读取一行数据 (对应 WASI fd_read)
    fn stdin_read_line(&self) -> String;
    
    /// 退出进程 (对应 WASI proc_exit)
    fn proc_exit(&self, code: i32) -> !;
    
    /// 获取当前时间 (对应 WASI clock_time_get)
    fn clock_now(&self) -> f64;
    
    /// 获取环境变量 (对应 WASI environ_get)
    fn get_env(&self, key: &str) -> Option<String>;
    
    /// 读取文件内容 (对应 WASI fd_read)
    fn fs_read_to_string(&self, path: &str) -> Result<String, String>;

    /// 写入文件内容 (对应 WASI fd_write)
    fn fs_write(&self, path: &str, content: &str) -> Result<(), String>;

    /// 判断文件是否存在
    fn fs_exists(&self, path: &str) -> bool;

    /// 删除文件 (对应 WASI path_unlink_file)
    fn fs_remove_file(&self, path: &str) -> Result<(), String>;

    /// 获取随机数 (对应 WASI random_get)
    fn random_f64(&self) -> f64;

    /// 线程休眠 (对应 WASI poll_oneoff)
    fn sleep(&self, ms: u64);
}

/// 一个没有任何操作的平台实现，用于初始化
pub struct StubPlatform;

impl NyarPlatform for StubPlatform {
    fn stdout_write(&self, _msg: &str) {}
    fn stdin_read_line(&self) -> String {
        String::new()
    }
    fn proc_exit(&self, _code: i32) -> ! {
        std::process::exit(1)
    }
    fn clock_now(&self) -> f64 {
        0.0
    }
    fn get_env(&self, _key: &str) -> Option<String> {
        None
    }
    fn fs_read_to_string(&self, _path: &str) -> Result<String, String> {
        Err("StubPlatform: fs_read_to_string not implemented".to_string())
    }
    fn fs_write(&self, _path: &str, _content: &str) -> Result<(), String> {
        Err("StubPlatform: fs_write not implemented".to_string())
    }
    fn fs_exists(&self, _path: &str) -> bool {
        false
    }
    fn fs_remove_file(&self, _path: &str) -> Result<(), String> {
        Err("StubPlatform: fs_remove_file not implemented".to_string())
    }
    fn random_f64(&self) -> f64 {
        0.0
    }
    fn sleep(&self, _ms: u64) {}
}
