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
        /// 解析失败的原因分类；来自解析期注册表或合同校验补记。
        reason: crate::compiler::types::hir::HirCallResolutionFailureKind,
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
    /// 内建结构化数组 `[T]` 的 owner 声明在可见作用域内多候选，无法冻结唯一身份。
    ArrayOwnerAmbiguous {
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
    /// nominal 声明 + 数组字面量初始值缺少 `LiteralSequence` 合同实现事实。
    ///
    /// 集合字面量必须由声明期望类型经 `imply ...: LiteralSequence<T>` 事实驱动构造；
    /// 合同 trait 路径解析不到、声明类型无对应 impl、或 impl 槽位形状不齐时，
    /// 在字面量构造点拒绝失败闭合，禁止先构造 `Array` 再隐式转换。
    CollectionLiteralContractMissing {
        /// 未匹配到合同事实的声明类型标签。
        type_label: String,
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
    /// `[constructor]` 标注落在非 `unite` 的变体上。
    AutoConstructorAttributeOnNonUnityVariant {
        /// 变体所属聚合名。
        aggregate: String,
        /// 被标注的变体名。
        variant: String,
        /// 聚合声明类别（`enums` / `enum`）。
        declared_kind: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `[constructor]` 标注落在变体以外的声明上。
    AutoConstructorAttributeOnNonVariant {
        /// 被标注声明的类别。
        target_kind: String,
        /// 被标注声明的短名。
        name: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 同一变体重复标注 `[constructor]`。
    DuplicateAutoConstructorAttribute {
        /// 变体所属聚合名。
        aggregate: String,
        /// 被重复标注的变体名。
        variant: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 泛型类/结构体的声明体内声明了方法；单态 owner 的声明体方法已经支持，
    /// 泛型 owner 暂不支持（暂时的实现边界，不是语言禁令）。
    ClassBodyMethodDeclared {
        /// 声明方法所在的聚合名。
        aggregate: String,
        /// 被声明的方法短名。
        method: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// impl 成员声明面残留了无法对齐到 owner 声明的泛型 binder。
    ///
    /// 泛型 binder 必须在声明冻结期写定；调用点不得用 receiver 实参回填。
    /// 出现本错误说明 impl 头部/成员签名里写的名字既不属于 impl 头、owner 声明，
    /// 也不属于 trait 声明，身份无法冻结。
    ImplGenericBinderUnresolved {
        /// impl target 的 owner 声明名（无法识别时携带占位说明）。
        declaration: String,
        /// 残留未绑定泛型的成员位置（成员名或 impl 头位置）。
        member: String,
        /// 未绑定 binder 的泛型形参名。
        generic: String,
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
    /// 阶段边界收口：诊断暂存汇中仍有 Error 级可恢复诊断，拒绝带着被丢弃的项继续物化。
    /// 全部诊断留在暂存汇中由失败路径统一渲染；本变体只携带首条的归属事实。
    PendingErrorsRejected {
        /// 暂存汇中 Error 级诊断总数（含超出上限被丢弃的）。
        count: usize,
        /// 首条 Error 诊断的源码身份。
        source: DiagnosticSource,
        /// 首条 Error 诊断的定位。
        span: DiagnosticSpan,
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
    /// 导入声明没有可解析的路径：语法未被前端识别（例如已废弃的 `using!`）。
    ///
    /// Oak 解析器对无法识别的导入语法不做诊断，只产出空路径的 `UsingDeclaration`；
    /// HIR 不得把它翻译成空 `HirImport` 静默放行，必须在此 fail-closed。
    UnrecognizedImport {
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `using!` 出现在非主空间 `namespace!` 位置。
    ///
    /// `using!` 是主空间专属导入，只能放在 `namespace!` 内。
    PrimaryImportOutsidePrimaryNamespace {
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 同一编译工程声明了多处 `namespace!`。
    ///
    /// `namespace!` 全工程唯一，不得取第一个或按文件顺序合并。
    DuplicatePrimaryNamespace {
        /// 已声明的主空间名字，按出现顺序。
        namespaces: Vec<String>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 传入的 HIR wire 版本与当前合同不符。
    ///
    /// 旧序列表省略新增字段时版本缺省为 `0`；此处结构化拒绝，不做静默兼容。
    HirWireVersionRejected {
        /// 报文携带的版本。
        found: u32,
        /// 当前合同要求的版本。
        expected: u32,
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
    /// 表达式 lowering 递归深度超过守卫上限。
    ///
    /// 该错误只表达「嵌套过深」这一结构化事实，不携带任何降级产物：
    /// 调用方必须失败闭合，不得忽略或继续 lowering。
    ExpressionNestingExceeded {
        /// 触发时已进入的 lowering 层级。
        depth: usize,
        /// 允许的最大层级。
        limit: usize,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// 单态化固定点超过上限。
    ///
    /// 递归泛型或开放克隆会让 `(声明, 代入)` 组合无界膨胀；该错误只表达
    /// 「超过上限」这一结构化事实，调用方必须失败闭合，不得忽略超限继续铸造，
    /// 也不许静默截断实例表。
    GenericInstantiationLimitExceeded {
        /// 触发的上限类别。
        kind: GenericInstantiationLimitKind,
        /// 允许的上限。
        limit: usize,
        /// 触发时的实际规模。
        actual: usize,
        /// 实例增长最快的声明 top-N：声明限定名与该声明已铸造的实例数。
        top_growth: Vec<(String, usize)>,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span（增长最快的声明的冻结 span）。
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
    /// 单引号捕获字面量的语义判定失败（字符/字节/raw 归属与转义解码）。
    InvalidSingleQuoteLiteral {
        /// 结构化失败原因。
        failure: SingleQuoteLiteralFailure,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span（指向出错的转义序列或整个字面量）。
        span: DiagnosticSpan,
    },
    /// 整数字面量按"符号 + 量级"解释后超出 `i64` 可表示范围。
    IntegerLiteralOutOfRange {
        /// 源码写出的字面量（含符号）。
        literal: String,
        /// 量级的十进制文本；量级本身超出 `u64` 时保留源码量级文本。
        magnitude: String,
        /// 触发的范围：`signed_64` 或 `unsigned_64_magnitude`。
        range: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断定位 span（指向整个字面量）。
        span: DiagnosticSpan,
    },
    /// 元组类型表达式的标签形态不合法。
    ///
    /// 元组要么全命名、要么全不命名；部分命名会让类型身份里出现无名占位，
    /// 属于必须拒绝的非法状态。全命名时标签不得重复。
    InvalidTupleLabeling {
        /// 结构化失败原因。
        failure: TupleLabelingFailure,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `[T; extent]` 的类型位 extent 不合法。
    ///
    /// extent 的合法形态只有整数字面量与整数元组字面量：前者是栈 `[T; N]`，
    /// 后者是堆形状包装 `[T; (W, H)]`。其它表达式、负数量级、零维度与
    /// 超出支持维数都必须拒绝，不得静默折叠成别的长度。
    InvalidArrayExtent {
        /// 结构化失败原因。
        failure: ArrayExtentFailure,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span。
        span: DiagnosticSpan,
    },
    /// `[export(case: ...)]` 的 `case` 取值不在合同枚举内。
    ///
    /// 合法取值集合与当前非法值都进入诊断参数，定位落在 `case` 实参表达式上：
    /// 非法值不得因整条导出合同静默返回 `None` 而退化成下游「缺少导出」的二级错误。
    InvalidExportCase {
        /// 注解给出的原始取值文本（非字符串字面量记为 `<non-string literal>`）。
        value: String,
        /// 诊断引用的源码或 artifact 身份。
        source: DiagnosticSource,
        /// 诊断主定位 span（`case` 实参表达式）。
        span: DiagnosticSpan,
    },
}

/// 固定数组 extent 的结构化失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayExtentFailure {
    /// extent 不是整数字面量或整数元组字面量。
    NotLiteral,
    /// 整数字面量量级超出 `usize` 可表示范围。
    LengthNotRepresentable,
    /// 带负号的 extent。
    NegativeLength,
    /// 元组形状的某一维为 0。
    ZeroDimension,
    /// 元组形状的维数不在 1..=3。
    UnsupportedDimensionCount {
        /// 实际写出的维数。
        count: usize,
    },
}

/// 元组标签形态的结构化失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TupleLabelingFailure {
    /// 同一元组里既有带名元素又有不带名元素。
    Mixed,
    /// 全部带名但标签重复。
    DuplicateLabel {
        /// 重复的标签文本。
        name: String,
    },
}

/// 单引号捕获字面量的结构化失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleQuoteLiteralFailure {
    /// 未识别的前缀标识符：单引号捕获只定义 `r`（raw）与 `b`（字节）。
    UnknownPrefix {
        /// 源码写出的前缀。
        prefix: String,
    },
    /// 空字节字面量 `b''`。
    EmptyByteLiteral,
    /// 字节字面量解码后不是恰好一个字节。
    ByteLiteralNotSingleByte {
        /// 解码得到的字节数。
        count: usize,
    },
    /// 字节字面量的裸字符必须是 ASCII；更宽的值请写 `\xNN`。
    ByteLiteralNotAscii {
        /// 源码写出的字符。
        character: String,
    },
    /// 字节字面量不接受 `\u{...}` 转义，请写 `\xNN`。
    UnicodeEscapeInByteLiteral,
    /// 非法转义序列（如 `\q`）。
    InvalidEscapeSequence {
        /// 出错的转义文本（含反斜杠）。
        escape: String,
    },
    /// `\u{...}` 转义不是合法 Unicode 码位（空、非十六进制或超过 0x10FFFF）。
    InvalidUnicodeEscape {
        /// 出错的转义文本（含 `\u{` 与 `}`）。
        escape: String,
    },
    /// 内容以孤立反斜杠结束；按捕获优先语义，转义不能跨界救定界符。
    DanglingEscape,
    /// 字符字面量超出 BMP：`char` 是 u16 code unit，不能静默截断。
    CharacterBeyondBmp {
        /// 解码得到的码位。
        value: u32,
    },
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

/// 单态化固定点超限的类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericInstantiationLimitKind {
    /// 铸造的 `(声明, 代入)` 实例总数超过上限。
    Instances,
    /// 固定点轮次超过上限（仍有新实例产生，未见收敛趋势）。
    Rounds,
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
