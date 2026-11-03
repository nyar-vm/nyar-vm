//! 宿主时间通道：墙钟毫秒、单调毫秒与睡眠。
//!
//! 两个时钟语义严格分开：`epoch_millis` 走系统墙钟（可被校时回拨），
//! `monotonic_millis` 只依赖进程内 `Instant` 基准，**不得回退**，也不得用墙钟实现。

use std::{
    sync::OnceLock,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::error::OdinRuntimeError;

/// 进程单调时钟基准：首次查询时固定，此后只前进。
static MONOTONIC_BASE: OnceLock<Instant> = OnceLock::new();

/// Unix 纪元毫秒（墙钟）。
///
/// 系统时钟早于纪元时返回 0（不 panic、不回绕）；正常路径按毫秒向下取整。
pub fn epoch_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_millis().min(i64::MAX as u128) as i64).unwrap_or(0)
}

/// 单调时钟毫秒：从进程内首次调用起算，连续取值不小于前值。
pub fn monotonic_millis() -> i64 {
    let base = MONOTONIC_BASE.get_or_init(Instant::now);
    base.elapsed().as_millis().min(i64::MAX as u128) as i64
}

/// 睡眠给定毫秒数并原样返回该值。
///
/// 负值结构化失败（不 clamp 成 0）；零值直接返回。
pub fn sleep_ms(ms: i32) -> Result<i32, OdinRuntimeError> {
    if ms < 0 {
        return Err(OdinRuntimeError::NegativeArgument { operation: "sleep_ms", value: ms as i64 });
    }
    thread::sleep(Duration::from_millis(ms as u64));
    Ok(ms)
}
