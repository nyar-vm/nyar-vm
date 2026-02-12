use std::any::Any;
use std::fmt::Debug;

/// 平台抽象接口
/// 允许 VM 在不同环境（CLI, WASM, Embedded）中运行
pub trait NyarPlatform: Send + Sync + Debug + Any {
    /// 打印到标准输出
    fn stdout_write(&self, msg: &str);
    /// 打印到标准错误
    fn stderr_write(&self, msg: &str);
    /// 获取当前时间戳（毫秒）
    fn now_ms(&self) -> u64;
    /// 睡眠指定毫秒数
    fn sleep_ms(&self, ms: u64);
    /// 让出 CPU 时间片，如果是异步运行时则执行协同式挂起
    fn yield_now(&self);
}

/// 空操作平台（用于测试或无副作用环境）
#[derive(Debug, Default)]
pub struct NoopPlatform;

impl NyarPlatform for NoopPlatform {
    fn stdout_write(&self, _msg: &str) {}
    fn stderr_write(&self, _msg: &str) {}
    fn now_ms(&self) -> u64 { 0 }
    fn sleep_ms(&self, _ms: u64) {}
    fn yield_now(&self) {
        std::thread::yield_now();
    }
}
