//! 目标 operator 能力绑定的冻结合同。
//!
//! `operator` 表示**目标后端能力**，不是语言前端层的 operator。一个 operator
//! 绑定可以是一条目标指令、多条指令组合、参数重排或常量注入，未来也可以表达
//! host call、ABI 转换或 allocation/runtime 操作；因此合同用配方
//! （[`TargetOperatorRecipe`]）描述发射步骤，而不是假定「一条指令」。
//!
//! 源码属性只在 [`TargetExecutionPlan`](crate::TargetExecutionPlan) 构造时解析一次，
//! 助记符必须解析为完整的 arity、操作数发射配方、操作数/结果载体与编码，写入
//! [`TargetOperatorPlan`]。backend 只编码这里已冻结的配方，不得再从助记符字符串
//! 推断 arity、操作数改写或结果类型，也不得为未冻结的绑定补默认指令。

use odin_bytecode::OdinHeadCode;

/// 目标标量载体。
///
/// `F32` 是**独立载体**，不是 `F64` 的别名，也不是 `I32` 的别名：
/// 32 位浮点的宽度、NaN payload 宽度与 `-0.0` 符号位都由它自己承诺。
/// 任何目标都不得把 `f32` 值存进 `F64` 槽再搬回，也不得借 `I32` 槽做数值转换
/// （位模式重解释是唯一允许的 `F32 ↔ I32` 通路，且必须显式冻结在配方里）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOperandType {
    /// 32 位整数载体。
    I32,
    /// 64 位整数载体。
    I64,
    /// 32 位浮点载体。
    F32,
    /// 64 位浮点载体。
    F64,
}

/// 已冻结配方中的一步发射动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOperandStep {
    /// 按实参顺序发射全部实参。
    Arguments,
    /// 发射一个载体化的整数常量；整数取负所需的零常量在此显式冻结。
    IntegerConstant {
        /// 常量在目标栈上的标量载体。
        operand_type: TargetOperandType,
        /// 常量值。
        value: i32,
    },
    /// 把栈顶 `i64` 截窄为目标原生的 `i32` 载体（`l2i` / `conv.i4`）。
    ///
    /// 仅用于 JVM / MSIL 这类「位移量操作数必须窄成 `i32`」的目标：声明签名携带
    /// `i64` 位移量，计划显式冻结截窄步骤，backend 只机械执行，不得按助记符推断。
    /// Wasm 的 `i64.shl`/`i64.shr_s`/`i64.shr_u` 两个操作数仍是 `i64`，不套用该步骤。
    NarrowTopI64ToI32,
}

/// 已冻结的 operator 发射配方。
///
/// operator 不保证是单条指令：配方是 backend 唯一允许执行的机械步骤序列。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOperatorRecipe {
    /// 按实参顺序发射全部实参，再执行冻结编码。
    Operands,
    /// 按给定顺序发射每一步，再执行冻结编码；backend 只机械执行配方，不按助记符
    /// 推断取负、零常量或减法。
    Steps(&'static [TargetOperandStep]),
}

/// 已冻结的 operator 编码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOperatorEncoding {
    /// WASM 单字节 opcode。
    Wasm(u8),
    /// OVM 头码。
    Ovm(OdinHeadCode),
    /// JVM 冻结码（算术码单指令展开；比较码由 JVM backend 展开为指令序列）。
    Jvm(crate::lowering::backends::jvm::JvmOperatorCode),
    /// CLR MSIL opcode（与数值载体无关，载体差异已冻结在配方中）。
    Clr(crate::transitional::msil::MsilOpcode),
}

/// 一个已冻结的目标 operator 计划。
///
/// 计划冻结调用点所需的全部物理事实：实参个数、操作数载体、结果载体、发射配方与
/// 编码。backend 不得在此基础上再做任何推断或补全。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetOperatorPlan {
    /// 冻结的实参个数；编码前必须与调用实参个数一致。
    pub arity: usize,
    /// 实参的目标标量载体。
    pub operand_type: TargetOperandType,
    /// 结果的目标标量载体。
    pub result_type: TargetOperandType,
    /// 操作数发射配方。
    pub recipe: TargetOperatorRecipe,
    /// operator 编码。
    pub encoding: TargetOperatorEncoding,
}

/// wasm GC 数组目标 operator 的冻结操作。
///
/// 数组 operator 不是单条 opcode：`array.get`/`array.set` 需要先按元素类型
/// `ref.cast` 到具体 arraytype，`array.new` 的 arraytype 也随元素类型变化。
/// arraytype 索引**不冻结**在计划里——它是模块装配期 RepresentationPlan 注册表
/// （`gc_array_type_indices`）的事实，与 MIR `ArrayNew`/`ArrayGet`/`ArraySet`/
/// `ArrayLength` 臂共用同一解析入口；计划只冻结 arity 与操作身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmGcArrayOperation {
    /// `new<T>(length) -> [T]`：按元素 arraytype 分配默认值数组。
    New,
    /// `length<T>([T]) -> usize`：`ref.cast array` 后 `array.len`。
    Length,
    /// `get<T>([T], index) -> T`：`ref.cast` 元素 arraytype 后 `array.get`。
    Get,
    /// `set<T>([T], index, T) -> unit`：`ref.cast` 元素 arraytype 后 `array.set`。
    Set,
}

/// 一个已冻结的 wasm GC 数组 operator 计划。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmGcArrayOperatorPlan {
    /// 冻结的实参个数；编码前必须与调用实参个数一致。
    pub arity: usize,
    /// 冻结的数组操作身份。
    pub operation: WasmGcArrayOperation,
}

/// 计划对目标 operator 绑定的冻结状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOperatorBinding {
    /// 已冻结完整 operator 计划。
    Frozen(TargetOperatorPlan),
    /// 已冻结 wasm GC 数组 operator 计划（多步编码，arraytype 编码期解析）。
    FrozenWasmGcArray(WasmGcArrayOperatorPlan),
    /// 声明了目标 operator 绑定，但该目标没有可编码配方；emit 必须结构化失败。
    Unfrozen,
}
