//! 目标侧可执行合同（target-lane private）。
//!
//! 本模块承载 SSA/CFG 形状的可执行语义视图，归 emitter 私有；它不再是
//! `nyar-types` 的公共 MIR 边界，也不得被其他 crate 当作中性 API 消费。
//!
//! `IntrinsicOpcode`、Call 上的 God 字段，以及指令上的 `StorageKind` / `LayoutId`
//! **已删除**。物理布局只属于 RepresentationPlan，不得塞回共享指令面。

use std::collections::BTreeMap;

use ordered_float::OrderedFloat;

use nyar_types::{ItemInstanceId, NamePath, NyarType};

/// SSA value reference within an executable function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRef(pub u32);

/// Basic-block reference within an executable function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockRef(pub u32);

/// Origin of an SSA value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueOrigin {
    /// Function parameter.
    Parameter {
        /// Parameter index.
        index: usize,
        /// Parameter name.
        name: String,
    },
    /// Block parameter.
    BlockParameter {
        /// Owning block.
        block: BlockRef,
        /// Parameter name.
        name: String,
    },
    /// `let` binding.
    LetBinding {
        /// Binding name.
        name: String,
    },
    /// Pattern `mut pat`: mutable borrow binding.
    MutRefBinding {
        /// Binding name.
        name: String,
    },
    /// Pattern `pin mut pat`: pinned mutable borrow binding.
    PinMutRefBinding {
        /// Binding name.
        name: String,
    },
    /// Literal materialization.
    Literal,
    /// Path / symbol load.
    Path,
    /// Call result.
    CallResult,
    /// Compiler temporary.
    Temporary,
}

/// SSA value definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// Value id.
    pub id: ValueRef,
    /// How the value was introduced.
    pub origin: ValueOrigin,
}

/// Executable constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constant {
    /// Signed integer constant.
    Int(i64),
    /// IEEE-754 float64 constant.
    Float64(OrderedFloat<f64>),
    /// Boolean constant.
    Bool(bool),
    /// UTF-8 text constant. There is no unqualified language-level `string`;
    /// the encoding is part of the Semantic MIR contract.
    Utf8(String),
    /// UTF-16 text constant. This remains distinct from [`Self::Utf8`] even
    /// when a backend happens to use one physical carrier for both forms.
    Utf16(String),
    /// Unit constant.
    Unit,
    /// Nullable empty value (`T?` has no payload here). The context type on
    /// `LoadConstant` states which `T?` this empty reference belongs to;
    /// it is never a nominal `Option::None` variant.
    Null,
}

impl From<f64> for Constant {
    fn from(value: f64) -> Self {
        Self::Float64(OrderedFloat(value))
    }
}

/// Instruction / terminator operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// SSA value.
    Value(ValueRef),
    /// Immediate constant.
    Constant(Constant),
    /// Compiler-resolved callable identity.
    Item(ItemInstanceId),
    /// Named symbol path.
    Symbol(NamePath),
}

/// 可执行指令（Semantic MIR 信封的薄镜像）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    /// Stable instruction identity.
    pub id: nyar_types::InstructionId,
    /// SSA results (0..n); types live in the value table.
    pub results: Vec<ValueRef>,
    /// Instruction payload (transitional name; target MirOperation).
    pub kind: InstructionKind,
    /// Source or synthetic provenance.
    pub provenance: nyar_types::ProvenanceId,
}

impl Instruction {
    /// Wrap a kind in a synthetic envelope (tests / transitional emitters).
    pub fn from_kind(kind: InstructionKind) -> Self {
        Self {
            id: nyar_types::InstructionId::from_index(0).expect("stub InstructionId"),
            results: Vec::new(),
            kind,
            provenance: nyar_types::ProvenanceId::from_index(0).expect("synthetic ProvenanceId"),
        }
    }
}

/// Executable instruction kinds (MIR-shaped, platform types only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionKind {
    /// Materialize a constant.
    LoadConstant {
        /// Constant payload.
        constant: Constant,
        /// Optional static type.
        ty: Option<NyarType>,
    },
    /// Load a named symbol.
    LoadSymbol {
        /// Symbol path.
        path: NamePath,
    },
    /// Copy an operand into the output.
    Copy {
        /// Source operand.
        source: Operand,
    },
    /// 按目标类型做表示转换（语言层显式 `as`）。
    ///
    /// 与 [`Self::Copy`] 本质不同：结果槽类型由 `ty` 决定；源与目标的物理表示
    /// 不一致时，后端必须发射真实转换指令，禁止当成布局兼容的 Copy 静默搬运。
    Cast {
        /// 源操作数。
        source: Operand,
        /// 结果类型（Semantic MIR 已冻结 `TypeId` 的目标投影）。
        ty: NyarType,
    },
    /// Store into a named local (`let` binding).
    StoreVar {
        /// Variable name.
        name: String,
        /// Value to store.
        value: Operand,
        /// Optional static type.
        ty: Option<NyarType>,
    },
    /// 调用 callee。
    ///
    /// **已移除、禁止回填的 God 字段：** `dispatch`、`witness`、`evidence`、
    /// `generic_function`、`generic_arguments`、`effect`、`receiver_kind`、
    /// `parameter_types`、`intrinsic_opcode`、`signature_complete`、`has_this`。
    Call {
        /// 可调用操作数（`ItemInstanceId` / 函数值）。
        callee: Operand,
        /// 语义顺序的实参。
        arguments: Vec<Operand>,
    },
    /// 具名函数作值：把前端冻结的 callable 实例物化为函数值。
    ///
    /// 结果 SSA 值的类型是 `NyarType::Function`（Semantic MIR 已登记）；物理载体
    /// （如 wasm funcref table 槽位下标）由目标后端决定，不得在此携带任何后端
    /// 私有编码。实例必须已在链接事实中登记，缺失即结构化失败。
    LoadFunctionRef {
        /// 编译器冻结的可调用实例身份。
        instance: ItemInstanceId,
        /// 闭包捕获环境的聚合值；无捕获 lambda 与具名函数引用均为 `None`。
        environment: Option<Operand>,
        /// 被捕获名清单，仅用于目标侧失败诊断。
        captured: Vec<String>,
    },
    /// Construct a named aggregate / struct instance.
    StructNew {
        /// 已冻结的名义聚合身份。
        nominal: nyar_types::NominalInstanceId,
        /// 已冻结的字段身份与初始化值。
        fields: Vec<(nyar_types::FieldId, Operand)>,
    },
    /// Construct a tuple.
    TupleNew {
        /// 编译器已验证的完整 tuple 布局身份。
        layout_id: nyar_types::LayoutId,
        /// 元素值；arity / 类型来自结果 `TypeId`。
        fields: Vec<Operand>,
    },
    /// Read one element from a tuple by positional index.
    ///
    /// Tuple labels live only in the type identity; the physical layout is positional.
    TupleGet {
        /// 元组对象。
        object: Operand,
        /// 编译器已验证的完整 tuple 布局身份。
        layout_id: nyar_types::LayoutId,
        /// 来自元组类型身份的位置索引。
        index: u32,
    },
    /// Copy an aggregate by layout.
    AggregateCopy {
        /// 编译器已验证的源/目标聚合布局身份。
        layout_id: nyar_types::LayoutId,
        /// Source aggregate.
        source: Operand,
        /// Destination aggregate.
        dest: Operand,
    },
    /// Read a field.
    FieldGet {
        /// Object / aggregate.
        object: Operand,
        /// 已冻结的字段身份。
        field: nyar_types::FieldId,
    },
    /// Write a field.
    FieldSet {
        /// Object / aggregate.
        object: Operand,
        /// 已冻结的字段身份。
        field: nyar_types::FieldId,
        /// Value to write.
        value: Operand,
    },
    /// Construct a value of an explicitly declared nominal sum variant.
    SumNew {
        /// 已冻结的 sum 名义实例身份。
        nominal: nyar_types::NominalInstanceId,
        /// 已冻结的 variant 身份。
        variant: nyar_types::VariantId,
        /// Payload value for a payload-bearing variant.
        payload: Option<Operand>,
    },
    /// Extract a payload from a declared nominal sum variant.
    ///
    /// This is distinct from aggregate field access: the sum and variant
    /// identity are semantic metadata, never inferred from field spellings.
    SumPayloadGet {
        /// 已冻结的 sum 名义实例身份。
        nominal: nyar_types::NominalInstanceId,
        /// 已冻结的 variant 身份。
        variant: nyar_types::VariantId,
        /// Sum receiver.
        object: Operand,
    },
    /// Test whether a sum value is currently the given declared variant.
    ///
    /// Carries the same nominal identity as [`Self::SumNew`] / [`Self::SumPayloadGet`].
    /// Must not be lowered as a field named `tag` discovered from a physical carrier.
    SumVariantIs {
        /// 已冻结的 sum 名义实例身份。
        nominal: nyar_types::NominalInstanceId,
        /// 已冻结的 variant 身份。
        variant: nyar_types::VariantId,
        /// Sum receiver.
        object: Operand,
    },
    /// Pattern probe for handler / case dispatch.
    PatternMatch {
        /// Scrutinee value.
        value: Operand,
        /// Debug-only pattern description (not a language HIR pattern).
        pattern_debug: String,
    },
    /// 运行时长度的语言数组构造。使用完整 `array_type`，而非仅元素类型。
    ArrayNew {
        /// Full array type (`Array<T>` / …); transitional stand-in for `TypeId`.
        array_type: NyarType,
        /// Runtime length.
        length: Operand,
        /// Prescribed initialization (no silent “fill later”).
        initialization: ArrayInitialization,
    },
    /// Construct language array from a complete element sequence (replaces FixedArrayNew/ArrayLiteral).
    ArrayFromElements {
        /// 定长值聚合的布局身份；动态数组没有聚合布局，必须为 `None`。
        ///
        /// 平台若需要定长数组的物理 body，必须显式处理缺失事实，不得用
        /// dummy 身份占位。
        layout_id: Option<nyar_types::LayoutId>,
        /// Full array type; fixed length lives in the type, not a parallel `usize`.
        array_type: NyarType,
        /// Element values in evaluation order already sequenced by SSA/CFG.
        elements: Vec<Operand>,
    },
    /// 从数组读取元素。
    ArrayGet {
        /// 数组操作数。
        array: Operand,
        /// 下标操作数。
        index: Operand,
    },
    /// 向数组写入元素。
    ArraySet {
        /// 数组操作数。
        array: Operand,
        /// 下标操作数。
        index: Operand,
        /// 要写入的元素值。
        value: Operand,
    },
    /// 读取数组长度。
    ArrayLength {
        /// 数组操作数。
        array: Operand,
    },
    /// Test whether a nullable value is null.
    NullIsNull {
        /// 可空值操作数。
        value: Operand,
    },
    /// Unwrap a non-null nullable payload.
    NullUnwrap {
        /// 已确认为非 null 的可空值操作数。
        value: Operand,
    },
}

/// Array slot initialization for [`InstructionKind::ArrayNew`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayInitialization {
    /// Language-defined default for the element type.
    Default,
    /// Fill every slot with this value.
    Fill(Operand),
}

/// Basic-block terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Return from the function.
    Return {
        /// Optional return value.
        value: Option<Operand>,
    },
    /// Unconditional jump with block arguments.
    Jump {
        /// Target block.
        target: BlockRef,
        /// Block arguments.
        arguments: Vec<Operand>,
    },
    /// Conditional branch.
    Branch {
        /// Branch condition.
        condition: Operand,
        /// Then target.
        then_target: BlockRef,
        /// Else target.
        else_target: BlockRef,
    },
    /// 可恢复的异常传播（语言 `raise` 的共享目标事实）。
    ///
    /// 语言拥有 `raise / yield / await` 等全部效果语义与验证；公共目标合同只保留
    /// 唯一已有后端落点的可恢复传播：payload 交给目标（wasm trap / OVM
    /// `PerformEffect`）后可在 `resume_target` 恢复。其余效果在语言适配层失败闭合，
    /// 不得回到公共枚举。
    Raise {
        /// Optional payload.
        payload: Option<Operand>,
        /// Resume target.
        resume_target: BlockRef,
    },
    /// State-machine dispatch on a state value.
    StateDispatch {
        /// State SSA value.
        state: ValueRef,
        /// `(state_id, target)` cases.
        cases: Vec<(u32, BlockRef)>,
        /// Default target.
        default_target: BlockRef,
    },
    /// Unreachable terminator.
    Unreachable,
}

/// Compile-time diagnostic collected while building an executable view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnostic {
    /// Pattern matching could not be lowered.
    PatternLoweringFailed {
        /// Human-readable reason.
        reason: String,
    },
}

/// Executable basic block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Block id.
    pub id: BlockRef,
    /// Debug label.
    pub label: String,
    /// Block parameters.
    pub parameters: Vec<ValueRef>,
    /// Instructions.
    pub instructions: Vec<Instruction>,
    /// Terminator.
    pub terminator: Terminator,
}

/// Backend-private executable function payload.
///
/// MIR-shaped during migration, but not a platform god IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableFunction {
    /// Function symbol.
    pub symbol: String,
    /// Return type.
    pub return_type: NyarType,
    /// Parameter types.
    pub param_types: Vec<NyarType>,
    /// SSA value static types.
    pub value_types: BTreeMap<ValueRef, NyarType>,
    /// Semantic MIR 已绑定的 SSA 值到聚合布局身份映射。
    pub value_layouts: BTreeMap<ValueRef, nyar_types::LayoutId>,
    /// Semantic MIR 已绑定的返回值布局身份（标量/无返回值为 `None`）。
    pub return_layout: Option<nyar_types::LayoutId>,
    /// Entry block.
    pub entry: BlockRef,
    /// SSA values.
    pub values: Vec<Value>,
    /// Blocks.
    pub blocks: Vec<Block>,
    /// Diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}
