//! Driver-side executable query interface.
//!
//! `emitter` should treat language executable structure as private and only
//! consume it through this query surface and the helper types defined here.

/// 语言类型视图别名。
pub type SemanticType = nyar::SemanticType;
/// 可执行值引用。
pub type ExecutableValueRef = crate::contracts::ValueRef;
/// 基本块引用。
pub type ExecutableBlockRef = crate::contracts::BlockRef;
/// 指令操作数。
pub type ExecutableOperand = crate::contracts::Operand;
/// 常量操作数。
pub type ExecutableConstant = crate::contracts::Constant;
/// 调用分派种类。
pub type ExecutableDispatchKind = crate::contracts::DispatchKind;
/// 存储种类。
pub type ExecutableStorageKind = crate::contracts::StorageKind;
/// 接收者传参方式。
pub type ExecutableReceiverPassingKind = crate::contracts::ReceiverPassingKind;
/// 单条可执行指令。
pub type ExecutableInstruction = crate::contracts::Instruction;
/// 指令种类。
pub type ExecutableInstructionKind = crate::contracts::InstructionKind;
/// 基本块终结符。
pub type ExecutableTerminator = crate::contracts::Terminator;
/// SSA 值描述。
pub type ExecutableValue = crate::contracts::Value;
/// 基本块。
pub type ExecutableBlock = crate::contracts::Block;
/// 函数级可执行语义。
pub type ExecutableFunction = crate::contracts::ExecutableFunction;

/// A driver-side view of function-level executable semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionView {
    /// 底层可执行函数合同。
    pub function: ExecutableFunction,
}

impl FunctionView {
    /// 返回函数符号名。
    pub fn symbol(&self) -> &str {
        &self.function.symbol
    }

    /// 返回全部基本块。
    pub fn blocks(&self) -> &[ExecutableBlock] {
        &self.function.blocks
    }
}
