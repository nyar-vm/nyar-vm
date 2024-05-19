//! Driver-side executable query interface.
//!
//! `emitter` should treat language executable structure as private and only
//! consume it through this query surface and the helper types defined here.

/// 语言类型视图别名。
pub type NyarType = nyar::NyarType;
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
/// 可执行层诊断。
pub type ExecutableDiagnostic = crate::contracts::Diagnostic;
/// SSA 值描述。
pub type ExecutableValue = crate::contracts::Value;
/// 挂起点元数据。
pub type ExecutableSuspendPoint = crate::contracts::SuspendPoint;
/// 帧布局描述。
pub type ExecutableFrameLayout = crate::contracts::FrameLayout;
/// 续延元数据。
pub type ExecutableContinuation = crate::contracts::Continuation;
/// `case` 链元数据。
pub type ExecutableCaseChain = crate::contracts::CaseChain;
/// 基本块。
pub type ExecutableBlock = crate::contracts::Block;
/// 挂起降低计划。
pub type ExecutableSuspendPlan = crate::contracts::SuspendLoweringPlan;
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

    /// 返回挂起点列表。
    pub fn suspend_points(&self) -> &[ExecutableSuspendPoint] {
        &self.function.suspend_points
    }

    /// 返回 `case` 链列表。
    pub fn case_chains(&self) -> &[ExecutableCaseChain] {
        &self.function.case_chains
    }
}

/// Minimal suspend metadata view required by suspend-aware backends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspendMetadataView {
    /// 所属函数符号。
    pub function_symbol: String,
    /// 挂起点列表。
    pub suspend_points: Vec<ExecutableSuspendPoint>,
    /// 帧布局列表。
    pub frame_layouts: Vec<ExecutableFrameLayout>,
    /// 续延列表。
    pub continuations: Vec<ExecutableContinuation>,
    /// `case` 链列表。
    pub case_chains: Vec<ExecutableCaseChain>,
}

impl SuspendMetadataView {
    /// 从可挂起函数提取最小挂起元数据视图。
    pub fn from_function(function: &ExecutableFunction) -> Option<Self> {
        let suspend_plan = function.suspend_plan.as_ref()?;
        // `suspend_plan` is canonical; still keep points/layouts as explicit slices for consumers.
        let _ = suspend_plan;
        Some(Self {
            function_symbol: function.symbol.clone(),
            suspend_points: function.suspend_points.clone(),
            frame_layouts: function.frame_layouts.clone(),
            continuations: function.continuations.clone(),
            case_chains: function.case_chains.clone(),
        })
    }
}
