//! 宿主随机通道：`xoshiro256**` 伪随机序列。
//!
//! 算法与状态：**xoshiro256\*\***（Blackman–Vigna），状态宽度 **256 位（4 × u64）**，
//! 播种用 SplitMix64 展开单个 u64 种子。输出为全宽原始随机字，不做取模或范围整形——
//! 范围整形属于 std `Generator` 层。
//!
//! 种子优先级：宿主显式设定 > 环境变量 `ODIN_VM_RAND_SEED` > 固定默认常量。
//! 环境变量非法（非十进制、负数、超出 u64）时结构化失败，不静默回落默认种子。

use std::{
    cell::{Cell, RefCell},
    env,
};

use crate::error::OdinRuntimeError;

/// 环境变量名：覆盖默认随机种子。
pub const RANDOM_SEED_ENV: &str = "ODIN_VM_RAND_SEED";

/// 默认固定种子：同一程序两次运行得到同一序列（确定性是特性，不是缺陷）。
pub const DEFAULT_SEED: u64 = 0x5EED_0F17_2A3B_4C5D;

thread_local! {
    static STATE: RefCell<Option<Xoshiro256>> = const { RefCell::new(None) };
    static EXPLICIT_SEED: Cell<Option<u64>> = const { Cell::new(None) };
}

/// xoshiro256** 状态。
struct Xoshiro256 {
    state: [u64; 4],
}

impl Xoshiro256 {
    /// 用 SplitMix64 把单个 u64 种子展开为 256 位状态。
    fn from_seed(seed: u64) -> Self {
        let mut mixer = seed;
        let mut state = [0u64; 4];
        for slot in &mut state {
            *slot = splitmix64_next(&mut mixer);
        }
        Self { state }
    }

    fn next(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let shift = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= shift;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }
}

/// SplitMix64 一步：推进 `mixer` 并返回混合后的 64 位值。
fn splitmix64_next(mixer: &mut u64) -> u64 {
    *mixer = mixer.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut value = *mixer;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// 宿主显式设种子：覆盖环境变量与默认值，并重置当前线程的随机状态。
pub fn set_seed(seed: u64) {
    EXPLICIT_SEED.with(|slot| slot.set(Some(seed)));
    STATE.with(|slot| *slot.borrow_mut() = Some(Xoshiro256::from_seed(seed)));
}

/// 解析种子文本：只接受十进制 `u64`；空串、非数字、负数、溢出一律结构化失败。
pub fn parse_seed(source: &'static str, text: &str) -> Result<u64, OdinRuntimeError> {
    text.parse::<u64>().map_err(|_| OdinRuntimeError::InvalidSeed { source, value: text.to_string() })
}

/// 按优先级解析种子：显式设定 > 环境变量 > 默认常量。
pub fn resolve_seed(explicit: Option<u64>, env_text: Option<&str>) -> Result<u64, OdinRuntimeError> {
    match (explicit, env_text) {
        (Some(seed), _) => Ok(seed),
        (None, Some(text)) => parse_seed(RANDOM_SEED_ENV, text),
        (None, None) => Ok(DEFAULT_SEED),
    }
}

/// 取下一个全宽 64 位随机字（首次调用时按种子优先级初始化当前线程状态）。
pub fn next_u64() -> Result<u64, OdinRuntimeError> {
    STATE.with(|slot| {
        let mut state = slot.borrow_mut();
        if state.is_none() {
            let explicit = EXPLICIT_SEED.with(Cell::get);
            let env_text = env::var(RANDOM_SEED_ENV).ok();
            let seed = resolve_seed(explicit, env_text.as_deref())?;
            *state = Some(Xoshiro256::from_seed(seed));
        }
        Ok(state.as_mut().map(Xoshiro256::next).unwrap_or_default())
    })
}

/// 给定种子、连续取 `count` 个值的序列（测试 / 诊断用；不触碰线程局部状态）。
pub fn sequence_from_seed(seed: u64, count: usize) -> Vec<u64> {
    let mut rng = Xoshiro256::from_seed(seed);
    (0..count).map(|_| rng.next()).collect()
}
