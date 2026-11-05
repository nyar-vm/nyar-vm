//! Module and documentation definitions for HIR.

use super::{
    HirEnum, HirFlags, HirFunction, HirImpl, HirSingleton, HirStatement, HirStruct, HirTrait, HirWidget, ValkyrieType,
    type_family::{HirTypeFamily, HirTypeFunction},
};
use crate::compiler::types::{Identifier, NamePath, SourceSpan};

/// HIR wire 格式版本。
///
/// 新增字段必须递增本版本：反序列化的旧报文缺少版本字段（缺省为 `0`）或版本
/// 与本常量不符时，必须在语义合同校验处结构化拒绝，不许静默接收。
pub const HIR_WIRE_VERSION: u32 = 2;

/// A module in HIR.
///
/// Modules are the top-level organizational unit in Valkyrie,
/// containing functions, types, and other definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirModule {
    /// The fully qualified name path of the module.
    pub name: NamePath,
    /// wire 格式版本；反序列化旧报文时缺省为 `0`，由语义合同校验拒绝。
    #[cfg_attr(feature = "serde", serde(default))]
    pub wire_version: u32,
    /// Resolver 分配的依赖组身份；与语义闭包 `DeclarationKey.dependency_key` 对齐。
    #[cfg_attr(feature = "serde", serde(default))]
    pub dependency_key: String,
    /// 本组主源码路径；与语义闭包 `DeclarationKey.source_path` 对齐。
    #[cfg_attr(feature = "serde", serde(default))]
    pub source_path: String,
    /// 该文件是否由 `namespace!` 声明主空间。
    #[cfg_attr(feature = "serde", serde(default))]
    pub primary_namespace: bool,
    /// Documentation for the module.
    pub doc: HirDocumentation,
    /// 文件作用域表：每个源文件的路径、`SourceID`、namespace、局部导入与声明名。
    ///
    /// 合并后的声明仍平铺在本模块；文件作用域只承载身份与导入归属，
    /// 不改变声明存储位置，也**不**用 `submodules` 塞文件。
    #[cfg_attr(feature = "serde", serde(default))]
    pub files: Vec<HirFileScope>,
    /// 主空间 `using!` 导入：全 unit 共享，不属于任何单文件的局部导入。
    #[cfg_attr(feature = "serde", serde(default))]
    pub primary_imports: Vec<HirImport>,
    /// Non-fatal compile warnings collected during lowering.
    #[cfg_attr(feature = "serde", serde(skip, default = "Vec::new"))]
    pub warnings: Vec<HirCompileWarning>,
    /// Nested submodules.
    ///
    /// 既有递归载体：expansion、type relation、emitter、reachability、trait system、
    /// generic binders 等消费者会递归读取。**不是**文件载体，不承载文件作用域；
    /// 文件身份只走 `files`。
    pub submodules: Vec<HirModule>,
    /// Functions defined in this module.
    pub functions: Vec<HirFunction>,
    /// Structs defined in this module.
    pub structs: Vec<HirStruct>,
    /// Enums defined in this module.
    pub enums: Vec<HirEnum>,
    /// Nominal sum definitions exported by resolved dependencies.  This is
    /// legacy migration storage. Formal compilation must use
    /// `imported_nominal_exports`, which retains exporter provenance.
    #[cfg_attr(feature = "serde", serde(default))]
    pub imported_enums: Vec<HirEnum>,
    /// Complete nominal metadata explicitly exported by resolved dependencies.
    /// Consumers may use an imported variant only when its exporter and layout
    /// are both available through this structured boundary.
    #[cfg_attr(feature = "serde", serde(default))]
    pub imported_semantic_exports: Vec<HirDependencySemanticExport>,
    /// Flags types defined in this module.
    pub flags: Vec<HirFlags>,
    /// Traits defined in this module.
    pub traits: Vec<HirTrait>,
    /// Impl blocks defined in this module.
    pub impls: Vec<HirImpl>,
    /// Type functions defined in this module.
    pub type_functions: Vec<HirTypeFunction>,
    /// Type families defined in this module.
    pub type_families: Vec<HirTypeFamily>,
    /// Widgets defined in this module.
    pub widgets: Vec<HirWidget>,
    /// Singletons defined in this module.
    pub singletons: Vec<HirSingleton>,
    /// Top-level statements in this module.
    pub statements: Vec<HirStatement>,
    /// `type Alias = Target` declarations in this module.
    pub type_aliases: Vec<HirTypeAlias>,
    /// 由调用点代入在 frontend 内铸造的泛型 callable 实例。
    ///
    /// 声明本身保持开放（`instance` 为空），实例承载已代入的函数体与签名；
    /// MIR lowering 只发射本表与声明期已闭合的 callable。
    #[cfg_attr(feature = "serde", serde(default))]
    pub instantiated_callables: Vec<HirInstantiatedCallable>,
    /// 组内逐文件诊断源表：`SourceID.version_id` → 源文件路径。
    ///
    /// 多文件源码组在 lowering 时按文件分配 `SourceID` 并登记本表；诊断按表达式
    /// span 携带的 `SourceID` 回本表取真实文件，而不是整组共享入口文件身份。
    #[cfg_attr(feature = "serde", serde(default))]
    pub diagnostic_sources: std::collections::BTreeMap<u32, String>,
    /// 调用解析失败注册表：解析器在 `resolved` 留空的调用点登记结构化失败事实，
    /// 合同校验按调用点 span 回查，诊断不再依赖占位符或重新推导。
    #[cfg_attr(feature = "serde", serde(default))]
    pub call_resolution_failures: Vec<HirCallResolutionFailure>,
}

/// 一次调用解析失败的结构化事实。
///
/// 由重载解析在失败现场登记；合同校验只消费，不重新推导。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirCallResolutionFailure {
    /// 失败调用点的源码区间（含 lowering 分配的源文件身份）。
    pub span: SourceSpan,
    /// 用户可读的 callee 形态（`set.is_empty`、`next`、`<fn value>` 等）。
    pub callee: String,
    /// 失败原因分类。
    pub reason: HirCallResolutionFailureKind,
}

/// 调用解析失败的原因分类。
///
/// 解析器只能给出它在失败现场能证明的事实；候选集可见性过滤等
/// 不可观察的中间状态不在这里虚构。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirCallResolutionFailureKind {
    /// 名字未找到：作用域内没有任何以该名注册的 callable 候选。
    NameNotFound,
    /// 接收者类型未定：成员调用的对象表达式推不出类型。
    ReceiverTypeUnknown {
        /// 成员名。
        member: String,
        /// 同名候选数：接收者未定时也必须点名候选集，不得静默选首个。
        candidates: usize,
    },
    /// 候选集为空：接收者类型已知，但没有任何以该成员名注册的候选。
    NoMethodCandidate {
        /// 成员名。
        member: String,
        /// 接收者类型的用户可读形态。
        receiver: String,
    },
    /// 有同名候选，但实参/泛型代入后无一存活。
    NoViableOverload {
        /// 同名候选数。
        candidates: usize,
    },
    /// 标量族混比：`char`/`u8` 一侧与整型一侧直接比较，没有隐式统一。
    MixedScalarComparison {
        /// 左操作数类型的用户可读形态。
        left: String,
        /// 右操作数类型的用户可读形态。
        right: String,
    },
    /// 显式泛型实参未结算（仍含开放 binder），无法代入候选。
    GenericsUnresolved,
    /// callee 是计算得到的函数值，没有可命名符号。
    ComputedCallee,
    /// 候选已绑定，但 `where` 证据拒绝具体代入，实例未闭合（合同校验期补记）。
    InstanceNotClosed,
    /// for-in 源缺少迭代协议 trait 证据：既不是 `IntoIterator`，也不是 `Iterator`。
    ///
    /// for-in 的义务由 trait 声明身份结算；同名成员方法不构成证据。
    ForInProtocolUnsatisfied {
        /// 迭代源类型的用户可读形态。
        source: String,
    },
    /// 多个显式 impl 候选同具体度并存，无法选出唯一 witness。
    ///
    /// 与 `TraitSatisfactionError::AmbiguousExplicitImpls` 同一判定语义：
    /// 不得静默取首个、按声明顺序或按来源排序挑选。
    AmbiguousExplicitImpls {
        /// 并列的显式 impl 候选数。
        candidates: usize,
    },
    /// 解析器未留下失败事实：出现即表示诊断链路自身有缺口，而不是用户代码的新类别。
    FailureUnrecorded,
}

impl HirCallResolutionFailureKind {
    /// 机器可读的稳定分类标签，供诊断参数与统计分组使用。
    pub fn tag(&self) -> &'static str {
        match self {
            Self::NameNotFound => "name-not-found",
            Self::ReceiverTypeUnknown { .. } => "receiver-type-unknown",
            Self::NoMethodCandidate { .. } => "no-method-candidate",
            Self::NoViableOverload { .. } => "no-viable-overload",
            Self::MixedScalarComparison { .. } => "mixed-scalar-comparison",
            Self::GenericsUnresolved => "generics-unresolved",
            Self::ComputedCallee => "computed-callee",
            Self::InstanceNotClosed => "instance-not-closed",
            Self::ForInProtocolUnsatisfied { .. } => "for-in-protocol-unsatisfied",
            Self::AmbiguousExplicitImpls { .. } => "ambiguous-explicit-impls",
            Self::FailureUnrecorded => "failure-unrecorded",
        }
    }

    /// 用户可读的原因句子，接在 `unresolved call .. in ..` 之后。
    pub fn describe(&self) -> String {
        match self {
            Self::NameNotFound => "no callable with this name is in scope".to_string(),
            Self::ReceiverTypeUnknown { member, candidates } => {
                format!("receiver type is unknown, so member `{member}` cannot be resolved; {candidates} candidate(s) share the name")
            }
            Self::NoMethodCandidate { member, receiver } => {
                format!("no method candidate named `{member}` exists for receiver type `{receiver}`")
            }
            Self::NoViableOverload { candidates } => {
                format!("{candidates} candidate(s) share the name, but none matches the argument types")
            }
            Self::MixedScalarComparison { left, right } => {
                format!(
                    "`{left}` and `{right}` are different scalar families; cast explicitly (for example `c as u32`) or compare by code point"
                )
            }
            Self::GenericsUnresolved => "explicit generic arguments still contain open binders".to_string(),
            Self::ComputedCallee => "callee is a computed function value without a nameable symbol".to_string(),
            Self::InstanceNotClosed => {
                "candidate is bound, but `where` evidence rejected the concrete substitution, so the instance never closed".to_string()
            }
            Self::ForInProtocolUnsatisfied { source } => format!(
                "for-in source `{source}` lacks trait evidence: it does not implement `std.iterator::IntoIterator`"
            ),
            Self::AmbiguousExplicitImpls { candidates } => {
                format!("{candidates} explicit impl candidates share the same specificity, so no unique witness can be selected")
            }
            Self::FailureUnrecorded => {
                "the resolver rejected this call without recording a reason; this is a diagnostics gap in the compiler".to_string()
            }
        }
    }
}

/// A resolved `using` import in HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirImport {
    /// 是否为 `using!` 主空间导入（在所有 namespace 共享）。
    ///
    /// 与普通文件局部 `using` 是两个可区分身份，不得以空 `path` 或同名节点表达。
    #[cfg_attr(feature = "serde", serde(default))]
    pub primary: bool,
    /// Imported module path.
    pub path: NamePath,
    /// Optional module alias (`using path as Alias`).
    pub alias: Option<Identifier>,
    /// Selective import bindings.
    pub bindings: Vec<HirImportBinding>,
    /// Whether this is a glob import.
    pub glob: bool,
}

/// One selective import binding inside `using path.{ ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirImportBinding {
    /// Imported symbol name.
    pub name: Identifier,
    /// Optional local alias.
    pub alias: Option<Identifier>,
}

/// 一个源文件在编译 unit 内的作用域身份。
///
/// 文件作用域只描述「这个文件是谁、导入了什么、声明了哪些名字」；
/// 声明本身仍平铺在 `HirModule`，不为任何声明结构体增加 `file_id`。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirFileScope {
    /// Resolver 分配的源文件路径；单源码字符串编译时为空。
    #[cfg_attr(feature = "serde", serde(default))]
    pub path: String,
    /// 该文件 lowering 使用的 `SourceID.version_id`。
    #[cfg_attr(feature = "serde", serde(default))]
    pub source_id: u32,
    /// 本文件声明的命名空间路径；空表示无显式 `namespace`。
    #[cfg_attr(feature = "serde", serde(default))]
    pub namespace: Vec<Identifier>,
    /// 本文件局部 `using` 导入（不含主空间 `using!`）。
    #[cfg_attr(feature = "serde", serde(default))]
    pub imports: Vec<HirImport>,
    /// 本文件声明面导出的本地名（函数 / 结构 / 枚举 / trait / 类型别名）。
    ///
    /// 供按文件判定可见性：同 namespace 跨文件同名允许并存，但只有在
    /// 本文件声明或显式导入的名字对该文件可见。
    #[cfg_attr(feature = "serde", serde(default))]
    pub declarations: Vec<Identifier>,
}

/// A non-fatal compile warning attached to a module.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirCompileWarning {
    /// Stable warning code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Source location.
    pub span: SourceSpan,
}

/// A module-scoped type alias in HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirTypeAlias {
    /// Alias name.
    pub name: Identifier,
    /// Declaring namespace segments, excluding the alias name itself.
    ///
    /// Qualified references (`std.text.utf16`) must be matched against this structured
    /// identity instead of the last path segment alone.
    #[cfg_attr(feature = "serde", serde(default))]
    pub namespace: Vec<Identifier>,
    /// Generic parameter names (e.g. `T` in `type Alias<T> = Result<T, E>`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub generics: Vec<Identifier>,
    /// Resolved target type (may still contain the alias's own type parameters).
    pub target: ValkyrieType,
    /// Source span for error reporting.
    pub span: SourceSpan,
}

impl Default for HirModule {
    fn default() -> Self {
        Self {
            name: NamePath::new(vec![Identifier::new("main")]),
            wire_version: HIR_WIRE_VERSION,
            dependency_key: String::new(),
            source_path: String::new(),
            primary_namespace: false,
            doc: HirDocumentation::default(),
            files: Vec::new(),
            primary_imports: Vec::new(),
            warnings: Vec::new(),
            submodules: Vec::new(),
            functions: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            imported_enums: Vec::new(),
            imported_semantic_exports: Vec::new(),
            flags: Vec::new(),
            traits: Vec::new(),
            impls: Vec::new(),
            type_functions: Vec::new(),
            type_families: Vec::new(),
            widgets: Vec::new(),
            singletons: Vec::new(),
            statements: Vec::new(),
            type_aliases: Vec::new(),
            instantiated_callables: Vec::new(),
            diagnostic_sources: std::collections::BTreeMap::new(),
            call_resolution_failures: Vec::new(),
        }
    }
}

/// 一个由调用点代入铸造的泛型 callable 实例。
///
/// 唯一事实所有者是 frontend 单态化固定点：实例的签名、局部类型与聚合类型
/// 都已完成代入，不再含未绑定实参的泛型 binder。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirInstantiatedCallable {
    /// 已代入的函数体；其 `instance` 即本实例的 callable 身份。
    pub function: HirFunction,
    /// 代入后的 impl owner 类型（如 `Nominal{SwissTable, [i64, i64]}`）；自由函数为空。
    #[cfg_attr(feature = "serde", serde(default))]
    pub owner_type: Option<ValkyrieType>,
    /// 该实例所属 impl 块的 trait 声明身份；inherent impl 为空。
    #[cfg_attr(feature = "serde", serde(default))]
    pub trait_declaration: Option<nyar_types::ItemId>,
}

/// Structured semantic metadata exported by one resolved dependency module.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirDependencySemanticExport {
    /// Fully qualified module that owns these definitions.
    pub module: NamePath,
    /// Complete exported function signatures and call contracts.
    pub functions: Vec<HirFunction>,
    /// Complete aggregate declarations, including storage and field layout.
    pub structs: Vec<HirStruct>,
    /// Complete nominal sum definitions, including generic and variant layout.
    pub enums: Vec<HirEnum>,
    /// Exported trait contracts and witness requirements.
    pub traits: Vec<HirTrait>,
    /// Exported type aliases with resolved generic targets.
    pub type_aliases: Vec<HirTypeAlias>,
    /// Exported `imply` / trait-impl method contracts (signatures for overload).
    ///
    /// Bodies stay owned by the exporting module's MIR; consumers resolve calls
    /// through these contracts, then Stage1 links reachable MIR bodies.
    pub impls: Vec<HirImpl>,
}

impl HirModule {
    /// Iterates nominal sums available through the formal dependency-export
    /// boundary. The legacy `imported_enums` field is intentionally excluded.
    pub fn imported_nominal_enums(&self) -> impl Iterator<Item = &HirEnum> {
        self.imported_semantic_exports.iter().flat_map(|export| export.enums.iter())
    }
}

/// Documentation for HIR items.
///
/// Documentation is stored as a collection of lines, typically
/// extracted from doc comments (`///` or `/** */`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirDocumentation {
    /// The documentation lines.
    pub lines: Vec<String>,
}

impl HirDocumentation {
    /// Creates documentation from multiple lines.
    pub fn from_lines(lines: Vec<String>) -> Self {
        Self { lines }
    }

    /// Creates documentation from a single line.
    pub fn from_single(line: impl Into<String>) -> Self {
        Self { lines: vec![line.into()] }
    }

    /// Returns true if the documentation is empty.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}
