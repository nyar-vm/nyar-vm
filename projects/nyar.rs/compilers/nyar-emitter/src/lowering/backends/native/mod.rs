//! Native（x86-64 Windows PE）lowering 后端。
//!
//! 本模块只把 [`FragmentSubmission`](crate::FragmentSubmission) 中**已冻结**的事实
//! （函数、SSA 局部槽、冻结 native operator、块标签、终结符）机械展开为 x86-64
//! 机器码并写出 PE32+ 可执行文件；不做前端推断，也不为未冻结形状补造指令或运行时。
//!
//! 当前纵切只承诺：整数/布尔常量、`Copy`、`StoreVar`、同片段内部调用、块跳转与
//! 条件分支、`Return`，以及 [`NativeOperatorCode`] 冻结的算术/位/比较算子。
//! 聚合、数组、sum、闭包、可空、浮点、字符串、`Cast`、`LoadSymbol`、宿主导入调用、
//! 超过 4 个形参以及 `TargetOperatorRecipe::Steps` 一律结构化失败。

pub(crate) mod x64;

pub(crate) use x64::lower_fragment_to_native_executable;

/// 降低结果：完整 `PE` 字节与入口位置事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLoweredFragment {
    /// 完整 `PE32+` 可执行文件字节。
    pub executable: Vec<u8>,
    /// 入口点在 `.text` 内的偏移。
    pub entry_offset: u32,
    /// `.text` 节原始长度（未按文件对齐填充）。
    pub text_len: usize,
}

/// 已冻结的 native x86-64 operator 编码。
///
/// 算术/位/移位码要求冻结计划 `operand_type = I64` 且 `result_type = I64`；
/// 比较码要求 `operand_type = I64` 且 `result_type = I32`（`bool` 载体用 `I32`，
/// 比较结果按 `setcc` + `movzx` 归一为 0/1）。任何载体不符都由 backend 结构化拒绝，
/// 不做隐式截窄或符号扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeOperatorCode {
    /// `add rax, rcx`：64 位整数加法。
    I64Add,
    /// `sub rax, rcx`：64 位整数减法。
    I64Subtract,
    /// `imul rax, rcx`：64 位整数乘法。
    I64Multiply,
    /// `neg rax`：64 位整数取负（一元）。
    I64Negate,
    /// `and rax, rcx`：64 位按位与。
    I64BitAnd,
    /// `or rax, rcx`：64 位按位或。
    I64BitOr,
    /// `xor rax, rcx`：64 位按位异或。
    I64BitXor,
    /// `shl rax, cl`：64 位左移（位移量取 `cl`，x86 按 6 位取模）。
    I64ShiftLeft,
    /// `sar rax, cl`：64 位算术右移（有符号）。
    I64ShiftRight,
    /// `cmp` + `sete`：64 位相等比较，结果 0/1。
    I64CompareEqual,
    /// `cmp` + `setne`：64 位不等比较，结果 0/1。
    I64CompareNotEqual,
    /// `cmp` + `setl`：64 位有符号小于比较，结果 0/1。
    I64CompareLessThan,
    /// `cmp` + `setle`：64 位有符号小于等于比较，结果 0/1。
    I64CompareLessOrEqual,
    /// `cmp` + `setg`：64 位有符号大于比较，结果 0/1。
    I64CompareGreaterThan,
    /// `cmp` + `setge`：64 位有符号大于等于比较，结果 0/1。
    I64CompareGreaterOrEqual,
}

impl NativeOperatorCode {
    /// 该编码是否为比较码（结果为 0/1，载体 `I32`）。
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::I64CompareEqual
                | Self::I64CompareNotEqual
                | Self::I64CompareLessThan
                | Self::I64CompareLessOrEqual
                | Self::I64CompareGreaterThan
                | Self::I64CompareGreaterOrEqual
        )
    }

    /// 该编码冻结的实参个数（只有取负是一元）。
    pub fn arity(self) -> usize {
        match self {
            Self::I64Negate => 1,
            _ => 2,
        }
    }
}
