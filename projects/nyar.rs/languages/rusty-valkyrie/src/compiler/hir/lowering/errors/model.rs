use nyar_types::{DiagnosticArg, DiagnosticOwner, DiagnosticPhase, DiagnosticRow, DiagnosticSeverity, DiagnosticSource, DiagnosticSpan};

/// HIR lowering 阶段的结构化失败原因。
///
/// 每个 variant 对应 lowering 逻辑块里**一个语义场景**，携带结构化字段（不是已格式化的 String）。
/// 转换成 DiagnosticRow 时：
/// - phase 固定为 DiagnosticPhase::Hir
/// - owner 固定为 `DiagnosticOwner::frontend("rusty-valkyrie")`
/// - severity 固定为 DiagnosticSeverity::Error
/// - code 由 variant 确定性生成
/// - variant 字段映射到 args BTreeMap
// 该错误类型承载上游 `ParseError`（不可 `Clone`），因此不派生 `Clone` / `PartialEq`：
// 失败事实由各自生产者持有，HIR 只读转发。
#[derive(Debug)]
pub enum HirError {
    /// SMIR003: call contract 未解析（选择名 + 调用上下文）。
    UnresolvedCall {
        /// 未解析的选择名（如 `as_range`、`+`、`[]`）。
        selector: String,
        /// 失败所在的源码片段 / 函数名。
        context: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// SMIR003: constructor contract 未解析。
    UnresolvedConstructor {
        /// 未解析的构造名。
        name: String,
        /// 失败所在的源码片段。
        context: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 名义类型短名在作用域内歧义，无法冻结 declaration identity。
    NominalAmbiguous {
        /// 歧义名义类型名。
        name: String,
        /// 候选声明数量。
        candidate_count: usize,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// sum/unite 声明在作用域内歧义。
    SumUniteAmbiguous {
        /// 歧义声明名。
        name: String,
        /// 候选声明数量。
        candidate_count: usize,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 结构缺少 constructor_declaration，无法绑定名义类型。
    StructureMissingConstructor {
        /// 结构聚合全名（namespace::name）。
        qualified_name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// sum/unite 缺少 declaration，无法绑定名义类型。
    SumUniteMissingDeclaration {
        /// 声明名。
        name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 分配 discriminator 失败：必须是非负整数字面量 / 不能重复 tag。
    InvalidDiscriminator {
        /// 所属聚合名。
        aggregate: String,
        /// 失败场景。
        failure: DiscriminatorFailure,
        /// 涉及的变体名（None 表示 aggregate 级问题）。
        variant: Option<String>,
        /// 具体值（None 表示未知）。
        value: Option<String>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 源码组没有 body-lowering 可处理的源文件。
    NoBodySourceInGroup {
        /// 空组依赖身份。
        dependency_key: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// MIR 校验阶段的跨阶段升级桥接。
    ///
    /// 具体失败变体、code、source 与 span 由 `MirValidationError` 自己生产；
    /// 本变体只转发结构化错误，不在 HIR 侧重新解释或字符串化。
    MirValidationRejected(crate::compiler::mir::validation::MirValidationError),
    /// frontend 解析 / 暂存阶段失败。
    ///
    /// 原样转发 frontend 生产的 `ParseError`（parser、staging 边界的事实所有者），
    /// HIR 不重新拼装解析消息。
    FrontendRejected(crate::compiler::syntax::oak::ParseError),
    /// TGrammar 模板展开失败。
    TemplateExpansionFailed {
        /// 触发失败的模板构造。
        construct: TemplateConstruct,
        /// 结构化失败原因。
        failure: TemplateFailure,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `match` 源码恢复失败。
    MatchRecoveryFailed {
        /// 恢复阶段。
        stage: MatchRecoveryStage,
        /// 涉及的 pattern 文本（仅 pattern 恢复阶段携带）。
        pattern: Option<String>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 可打断控制流的操作出现在不允许它的上下文。
    ControlFlowContextViolation {
        /// 违规操作。
        operation: ControlFlowOp,
        /// 该操作要求的上下文。
        required: RequiredContext,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `fallthrough` 所在的 `case` arm 没有后继 arm。
    FallthroughWithoutNextArm {
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `fallthrough` 不在 `case` statement 体系内。
    FallthroughOutsideCaseChain {
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `fallthrough` 非法引用上一 arm 的 pattern 绑定。
    FallthroughBindingLeak {
        /// 被非法引用的绑定名。
        name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `break` / `continue` 找不到目标 loop。
    LoopTargetMissing {
        /// 跳转操作。
        operation: LoopJumpOperation,
        /// `break` 是否携带值。
        carries_value: bool,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 目标 loop 当前不接受 `break` 值。
    BreakValueRejected {
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `break` 值类型与目标 loop 已收敛的结果类型不兼容。
    BreakValueTypeMismatch {
        /// 实际值类型。
        actual: String,
        /// 目标 loop 已收敛的结果类型。
        expected: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `?` 的操作数不实现可传播协议（nullable / `Option` / `Result`）。
    TryPropagateOperandUnsupported {
        /// 操作数类型。
        operand_type: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `?` 所在函数的返回类型与操作数协议不匹配，且不在 `try` 作用域内。
    TryPropagateReturnMismatch {
        /// 操作数要求的协议。
        protocol: PropagateProtocol,
        /// 当前函数返回类型。
        return_type: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `?` 的错误类型与父返回类型的错误类型不兼容。
    TryPropagateErrorTypeMismatch {
        /// 操作数携带的错误类型。
        error_type: String,
        /// 父返回类型的错误类型。
        return_error: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `await` / `awake` / `block` 的操作数不是 `Future<T>` / `Promise<T>`。
    FutureControlOperandMismatch {
        /// 违规操作。
        operation: ControlFlowOp,
        /// 操作数类型。
        operand_type: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `return` / `yield` 缺少上下文要求的 carried value。
    ControlFlowValueRequired {
        /// 缺少值的操作。
        operation: ControlFlowOp,
        /// 上下文期望的类型。
        expected_type: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `return expr` 的值类型与函数返回类型不兼容。
    ReturnValueTypeMismatch {
        /// 实际值类型。
        actual: String,
        /// 函数返回类型。
        expected: String,
        /// 所在函数名（未知时为空）。
        function: Option<String>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `yield expr` 的值类型与生成器元素类型不兼容。
    YieldValueTypeMismatch {
        /// 实际值类型。
        actual: String,
        /// 函数返回类型。
        expected_return: String,
        /// 生成器 yield 元素类型。
        item_type: String,
        /// 所在函数名（未知时为空）。
        function: Option<String>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `yield from` 的操作数不是生成器协议。
    YieldFromOperandNotGenerator {
        /// 操作数类型。
        operand_type: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `yield from` 的元素类型与当前函数的 yield 元素类型不兼容。
    YieldFromItemTypeMismatch {
        /// 被委托生成器的元素类型。
        source_item: String,
        /// 当前函数返回类型。
        expected_return: String,
        /// 当前函数 yield 元素类型。
        expected_item: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 单段小写裸名字模式与变量绑定歧义。
    AmbiguousBareNamePattern {
        /// 歧义模式名。
        name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `let` 绑定的 pattern 可被拒绝（refutable 或不可判定）。
    RefutableLetPattern {
        /// pattern 的渲染文本。
        pattern: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `let` 显式声明类型与初始化表达式的静态类型不兼容。
    ///
    /// 整数档位按名义身份判定：不同位宽或有符号性不同的整数不得互相顶替；
    /// 整数字面量的宽度由声明类型决定，不构成此类不匹配。
    LetInitializerTypeMismatch {
        /// 初始化表达式的静态类型。
        actual: String,
        /// `let` 声明的类型。
        expected: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 对不可变 `let`（带初始化）绑定的赋值：赋值是对已绑定值的改值，须声明 `let mut`。
    AssignToImmutable {
        /// 被赋值的绑定名。
        name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 多个语义检查器各自生产的违规集合。
    SemanticCheckViolations {
        /// 按检查器分组的违规消息。
        checks: Vec<SemanticCheckFailure>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 值类型 copy 纪律违规集合。
    CopyDisciplineViolations {
        /// 逐条违规事实。
        violations: Vec<CopyDisciplineViolation>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 同一 namespace 下出现重复类型声明。
    DuplicateTypeDefinition {
        /// 重复声明的短名。
        name: String,
        /// 声明所在 namespace。
        namespace: String,
        /// 先前声明的种类。
        prior_kind: String,
        /// 本次声明的种类。
        current_kind: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 生成源码组缺少显式 producer 身份。
    SourceGroupProducerMissing {
        /// 涉及的源码组依赖身份（未知时为 `None`）。
        dependency_key: Option<String>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 生成源码组没有显式标记为入口。
    SourceGroupNotEntry {
        /// 源码组依赖身份。
        dependency_key: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 源码组依赖身份为空或与已提供组重复。
    SourceGroupIdentityInvalid {
        /// 请求的依赖身份（为空时携带空串）。
        dependency_key: String,
        /// 身份是否为空（否则为重复）。
        empty: bool,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 源码组依赖的组尚未在之前提供。
    SourceGroupDependencyUnavailable {
        /// 消费依赖的组。
        group: String,
        /// 缺失的依赖组。
        dependency: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 组内源码文件身份为空或重复。
    SourceFileIdentityInvalid {
        /// 所属组依赖身份。
        group: String,
        /// 请求的文件路径（为空时携带空串）。
        path: String,
        /// 身份是否为空（否则为重复）。
        empty: bool,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 入口组没有任何源码文件。
    EntryGroupWithoutFiles {
        /// 入口组依赖身份。
        group: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// Compiler 源码快照没有入口组（或缺少显式入口组）。
    SourceInputEntryGroupMissing {
        /// 是否缺少显式入口标记（否则快照完全为空）。
        explicit: bool,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 读取源码文件失败。
    SourceReadFailed {
        /// 请求的文件路径。
        path: String,
        /// 底层 IO 失败种类。
        kind: std::io::ErrorKind,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 语义源码组计划为空，无法确定链接根。
    EmptySemanticSourceGroupPlan {
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// frontend 拒绝源文件；原样转发其结构化解析失败。
    SourceFileRejected {
        /// 所属组依赖身份。
        dependency_key: String,
        /// 被拒绝的文件路径。
        path: String,
        /// frontend 生产的结构化解析失败。
        error: crate::compiler::syntax::oak::ParseError,
    },
    /// 依赖导出的语义事实不可用。
    DependencyExportUnavailable {
        /// 消费依赖的组。
        group: String,
        /// 缺失的依赖组。
        dependency: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 同一依赖身份被重复注册语义导出。
    DependencyExportIdentityCollision {
        /// 冲突的依赖身份。
        dependency_key: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 声明 / 实例 / 字段身份空间耗尽或索引非法。
    DeclarationIndexExhausted {
        /// 耗尽的索引空间。
        space: IndexSpace,
        /// 正在注册的声明所有者。
        owner: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 已分配的声明 / 实例 / 字段身份被重复注册。
    DeclarationIdentityDuplicated {
        /// 冲突的索引空间。
        space: IndexSpace,
        /// 声明所有者。
        owner: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 依赖声明的身份注册必须由完整 Compiler 源码闭包拥有。
    DependencyDeclarationsRequireSourceClosure {
        /// 已提供的依赖导出数量。
        imported_export_count: usize,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 命名 `union` 尚未进入 HIR lowering。
    UnionNotLowered {
        /// union 声明名。
        union_name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 必须在宏/模板展开后消失的 TGrammar 模板残留到 HIR lowering。
    UnexpandedTemplateItem {
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 根条目类型不属于 HIR lowering 的输入面。
    UnsupportedRootItem {
        /// Oak 根条目的稳定种类名。
        item_kind: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 下游 compile pipeline 拒绝已 lowering 的输入。
    ///
    /// 原样转发下游已冻结的诊断 code 与 module 身份，不在 HIR 侧重新拼装失败文本。
    PipelineRejected {
        /// 下游诊断 code 列表。
        codes: Vec<String>,
        /// owning module 符号。
        module: String,
        /// 下游 `DiagnosticRecord::message`（供 Legion / 矩阵排障；catalog 仍以 code 为准）。
        messages: Vec<String>,
    },
    /// 源码使用了已移除的旧内建类型别名。
    ///
    /// 旧别名不再是语言身份：源码必须改写为规范名（`{canonical}`）。
    LegacyBuiltinTypeAlias {
        /// 源码写出的旧别名拼写。
        alias: String,
        /// 应当改用的规范类型拼写。
        canonical: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 源码使用了不指定编码的歧义文本类型名。
    ///
    /// 文本类型必须写明编码（`utf8` / `utf16` / `c_str`），不能靠名字猜编码事实。
    AmbiguousTextTypeName {
        /// 源码写出的歧义文本类型拼写。
        name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 类型 lowering 时遇到无法解析的类型短名。
    ///
    /// 既不是当前作用域内已声明的 type 别名，也不是语言内建（`bool`/`utf8`/`unit`/`i64` 等）。
    /// 这种名字以前被错误地 fallback 成 `ValkyrieType::Named` 传递到 canonical 阶段，
    /// 但 canonical 已对 Named 直接返回 CAN003——现在在 lowering 阶段就结构化失败。
    TypeNotFound {
        /// 未解析的类型短名。
        name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `.vx` source 暂不支持。
    VxUnsupported,
    /// 源码组语义闭包失败：入口声明缺失、HostProvider 未解析等。
    /// 直接携带上游 `SemanticClosureError`，不在 lowering 阶段降级成字符串。
    SourceGroupClosureFailed(crate::compiler::semantic::closure::SemanticClosureError),
    /// DeriveInjector 注入失败：字段类型缺 trait、冲突等。
    /// 直接携带上游 `DeriveError` 列表，不在 lowering 阶段拼接成字符串。
    DeriveResolutionFailed {
        /// 结构化 derive 错误列表。
        errors: Vec<crate::compiler::expansion::derive::DeriveError>,
    },
    /// backend contract 拒绝声明面的互操作能力。
    /// 直接携带 backend contract 事实，不在 lowering 阶段重拼失败文本。
    BackendContractRejected(crate::compiler::emitter::backend_contract::interop::BackendContractError),
}

/// TGrammar 模板构造类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateConstruct {
    /// 元表达式 `<% ... %>`。
    MetaExpression,
    /// `loop _ in start..end` 常量范围展开。
    Loop,
    /// 按 `arch` 选择分支的 `match`。
    Match,
    /// 块内模板片段展开。
    BlockFragment,
}

/// TGrammar 模板展开的结构化失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateFailure {
    /// 元表达式为空。
    EmptyExpression,
    /// 表达式后仍有无法求值的语法。
    TrailingTokens,
    /// 表达式在未读完时结束。
    IncompleteExpression,
    /// 括号未闭合。
    UnclosedParenthesis,
    /// 出现无法求值的 token。
    UnevaluableToken,
    /// 整数字面量无法解析成 `i64`。
    IntegerLiteralUnparsable,
    /// 比较两侧类型不兼容。
    IncompatibleComparisonOperands,
    /// `loop` 头部短于 `_ in start..end`。
    LoopHeaderTooShort,
    /// `loop` 绑定名不是丢弃绑定 `_`。
    LoopBindingNotDiscard,
    /// `loop` 头部缺少 `in` 关键字。
    LoopMissingInKeyword,
    /// `loop` 头部在范围表达式之后仍有无法求值的语法。
    LoopHeaderTrailingTokens,
    /// `loop` 缺少范围表达式。
    LoopRangeMissing,
    /// `loop` 范围不是 `start..end` 形态。
    RangeShapeUnsupported,
    /// 范围端点不是整数字面量。
    RangeEndpointNotInteger,
    /// 范围起点大于终点。
    LoopRangeInvalid {
        /// 起点。
        start: i64,
        /// 终点。
        end: i64,
    },
    /// `match` 的 scrutinee 不是 `arch`。
    NonArchScrutinee,
    /// 展开结果不能作为块内语句保留。
    BlockFragmentUnsupportedNode {
        /// 无法保留的节点类别（稳定短名，不是 `Debug` 输出）。
        kind: String,
    },
    /// 元表达式重新解析后不是单个 `micro` 声明。
    FragmentNotMicroDeclaration {
        /// 重新解析产出的顶层项数量。
        produced_items: usize,
    },
}

/// `match` 源码恢复阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchRecoveryStage {
    /// 从源码 span 恢复 `match` 块内部文本。
    BlockInterior,
    /// `match` 块没有可恢复的 arm。
    Arms,
    /// 恢复 arm 的 pattern。
    Pattern,
    /// arm body 为空。
    ArmBody,
    /// arm body 不是表达式。
    ArmBodyExpression,
}

/// 可能打断控制流连续性的操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlowOp {
    /// `await`。
    Await,
    /// `awake`。
    Awake,
    /// `block`。
    Block,
    /// `yield`。
    Yield,
    /// `yield from`。
    YieldFrom,
    /// `resume`。
    Resume,
    /// `?`。
    TryPropagate,
    /// `fallthrough`。
    Fallthrough,
    /// `break`。
    Break,
    /// `continue`。
    Continue,
    /// `return`。
    Return,
}

/// 控制流操作要求的合法上下文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredContext {
    /// 必须位于 `catch arm body`。
    CatchArmBody,
    /// 不得位于 guard 中。
    OutsideGuard,
    /// 必须位于 async 作用域。
    AsyncScope,
    /// 必须位于允许阻塞的作用域。
    BlockingScope,
    /// 必须位于生成器或等价 handler 作用域。
    GeneratorScope,
}

/// loop 跳转操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopJumpOperation {
    /// `break`。
    Break,
    /// `continue`。
    Continue,
}

/// `?` 操作数要求的传播协议。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropagateProtocol {
    /// nullable 类型。
    Nullable,
    /// `Option<T>`。
    Option,
    /// `Result<T, E>`。
    Result,
}

/// 声明身份索引空间。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexSpace {
    /// `ItemId` 声明空间。
    Item,
    /// `ItemInstanceId` 实例空间。
    ItemInstance,
    /// `FieldId` 字段空间。
    Field,
}

/// 生产语义违规的检查器身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticChecker {
    /// 值类型语义检查。
    ValueType,
    /// `sealed match` 穷尽性检查。
    SealedMatch,
    /// 字面量穷尽性检查。
    LiteralExhaustiveness,
    /// 不可达 arm 检查。
    UnreachableArm,
    /// 复合模式穷尽性检查。
    CompositePatternExhaustiveness,
    /// singleton 检查。
    Singleton,
}

/// 单个检查器生产的违规消息集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticCheckFailure {
    /// 违规所属检查器。
    pub checker: SemanticChecker,
    /// 该检查器生产的违规消息。
    pub messages: Vec<String>,
}

/// 值类型 copy 纪律违规原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyDisciplineReason {
    /// 值类型含有引用类型字段。
    ReferenceTypedField,
    /// 参数是值类型但使用引用语义。
    ParameterReferenceSemantics,
    /// 参数使用含引用字段的值类型。
    ParameterNonCopyValueType,
    /// 返回类型是值类型但使用引用语义。
    ReturnReferenceSemantics,
    /// 返回类型使用含引用字段的值类型。
    ReturnNonCopyValueType,
}

/// `sum`/`unite` discriminator 分配失败的具体场景。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscriminatorFailure {
    /// `[tag(...)]` 或 `Variant = ...` 不是非负整数字面量。
    NotNonNegativeInteger,
    /// 两个变体得到相同 tag。
    DuplicateTag,
}

impl DiscriminatorFailure {
    /// 稳定场景名，用于诊断参数。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotNonNegativeInteger => "not_non_negative_integer",
            Self::DuplicateTag => "duplicate_tag",
        }
    }
}

/// 单条值类型 copy 纪律违规事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyDisciplineViolation {
    /// 违规所属函数 / 方法所有者。
    pub owner: String,
    /// 违规的参数名（仅参数违规携带）。
    pub item: Option<String>,
    /// 涉及的命名类型（仅类型传播违规携带）。
    pub type_name: Option<String>,
    /// 违规原因。
    pub reason: CopyDisciplineReason,
}
