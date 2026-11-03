//! 解释器执行期错误类型与诊断信息。

use std::fmt::{Display, Formatter};

use miette::Diagnostic;

/// Runtime error raised while executing Nyar bytecode.
#[derive(Debug, Clone, PartialEq, Eq, Diagnostic)]
pub enum OdinRuntimeError {
    /// Stack underflow.
    StackUnderflow,
    /// Unknown or unsupported opcode.
    UnknownOpcode(u8),
    /// Function index out of range.
    FunctionIndexOutOfRange(i32),
    /// Local variable index out of range.
    LocalIndexOutOfRange(i32),
    /// Global slot index out of range.
    GlobalIndexOutOfRange(i32),
    /// Constant pool index out of range.
    ConstantIndexOutOfRange(i32),
    /// Import table index out of range.
    ImportIndexOutOfRange(i32),
    /// Layouts 表下标越界（`ObjectNew`）。
    LayoutIndexOutOfRange(i32),
    /// 字段槽越界（`FieldGet` / `FieldSet`）。
    FieldSlotOutOfRange(i32),
    /// Entry function not found.
    EntryNotFound(String),
    /// `CallIntrinsic` 操作数不是已知 intrinsic 稠密下标。
    UnknownIntrinsic(i32),
    /// Type mismatch at runtime.
    TypeMismatch {
        /// Expected type name.
        expected: &'static str,
        /// Actual type name.
        actual: String,
    },
    /// 除零 / 模零 trap。
    DivisionByZero {
        /// 触发 trap 的运算名。
        operation: &'static str,
    },
    /// 有符号整数溢出 trap（如 `i64::MIN / -1`）：不得 panic 穿透进程。
    ArithmeticOverflow {
        /// 触发 trap 的运算名。
        operation: &'static str,
    },
    /// 码点下标越界（`utf8.slice`）：结构化失败，不 clamp 成空串或截断。
    IndexOutOfBounds {
        /// 触发失败的运算名。
        operation: &'static str,
        /// 请求的起始码点下标（可为负）。
        start: i64,
        /// 请求的码点个数（可为负）。
        count: i64,
        /// 文本实际码点数。
        length: u64,
    },
    /// 非法文本编码（非法 UTF-8 字节 / 孤立 UTF-16 代理）：fail-closed，不替换成 U+FFFD。
    InvalidUtf8 {
        /// 触发失败的运算名。
        operation: &'static str,
    },
    /// C 字符串没有终止 NUL：不得读到缓冲区外，也不得猜测长度。
    UnterminatedCStr {
        /// 触发失败的运算名。
        operation: &'static str,
    },
    /// 文本含内嵌 NUL，无法无损转成 C 字符串：拒绝而不是截断。
    EmbeddedNul {
        /// 触发失败的运算名。
        operation: &'static str,
    },
    /// `Trap` 指令被执行：前端声明为不可达的块在运行期真的到达了。
    TrapExecuted {
        /// 触发时正在执行的函数名。
        function: String,
        /// 触发时的指令下标（函数内 pc）。
        ip: u32,
    },
    /// 单次调用执行步数超预算：把"挂死"变成可归因的结构化失败。
    StepBudgetExceeded {
        /// 触发时正在执行的函数名。
        function: String,
        /// 触发时的指令偏移（函数内 pc）。
        ip: u32,
        /// 生效的步数预算。
        limit: u64,
    },
    /// Module load failure.
    ModuleLoad(String),
    /// Requested feature is not implemented (e.g. JIT while disabled).
    UnsupportedFeature(&'static str),
}

impl Display for OdinRuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StackUnderflow => write!(f, "stack underflow"),
            Self::UnknownOpcode(op) => write!(f, "unknown opcode: 0x{op:02X}"),
            Self::FunctionIndexOutOfRange(idx) => write!(f, "function index out of range: {idx}"),
            Self::LocalIndexOutOfRange(idx) => write!(f, "local index out of range: {idx}"),
            Self::GlobalIndexOutOfRange(idx) => write!(f, "global index out of range: {idx}"),
            Self::ConstantIndexOutOfRange(idx) => write!(f, "constant index out of range: {idx}"),
            Self::ImportIndexOutOfRange(idx) => write!(f, "import index out of range: {idx}"),
            Self::LayoutIndexOutOfRange(idx) => write!(f, "layout index out of range: {idx}"),
            Self::FieldSlotOutOfRange(idx) => write!(f, "field slot out of range: {idx}"),
            Self::EntryNotFound(name) => write!(f, "entry function not found: {name}"),
            Self::UnknownIntrinsic(index) => write!(f, "unknown intrinsic index: {index}"),
            Self::TypeMismatch { expected, actual } => {
                write!(f, "type mismatch: expected {expected}, got {actual}")
            }
            Self::DivisionByZero { operation } => write!(f, "{operation}: division by zero"),
            Self::ArithmeticOverflow { operation } => write!(f, "{operation}: integer overflow"),
            Self::IndexOutOfBounds { operation, start, count, length } => {
                write!(f, "{operation}: start {start} count {count} out of bounds for {length} code points")
            }
            Self::InvalidUtf8 { operation } => write!(f, "{operation}: invalid text encoding"),
            Self::UnterminatedCStr { operation } => write!(f, "{operation}: unterminated C string"),
            Self::EmbeddedNul { operation } => write!(f, "{operation}: embedded NUL cannot be represented as C string"),
            Self::TrapExecuted { function, ip } => {
                write!(f, "reached unreachable trap in function `{function}` at pc {ip}")
            }
            Self::StepBudgetExceeded { function, ip, limit } => {
                write!(f, "step budget exceeded in function `{function}` at pc {ip}: limit {limit} instructions per call")
            }
            Self::ModuleLoad(message) => write!(f, "failed to load module: {message}"),
            Self::UnsupportedFeature(feature) => write!(f, "unsupported feature: {feature}"),
        }
    }
}

impl std::error::Error for OdinRuntimeError {}
