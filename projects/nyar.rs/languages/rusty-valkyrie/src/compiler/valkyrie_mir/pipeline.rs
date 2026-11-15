//! 单向编译流的阶段 trait 合同与阶段模型。
//!
//! 实现位于各前端 crate 与后端 crate；本模块只定义阶段合同与阶段顺序。
//! 阶段与阶段诊断是 **Valkyrie 自己的 pipeline 事实**，不属于公共 `nyar-types`；
//! 诊断生产入口另有 `DiagnosticPhase` + `DiagnosticRow`，两套模型职责不同、不混用。

use super::ValkyrieMirProgram;
use super::representation::RepresentationPlan;
use nyar_types::LinkedSemanticProgram;

/// 语义 pipeline 阶段。
///
/// 面向 Semantic MIR pipeline 的语义阶段划分（M1/M2/Link/Representation 等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompileStage {
    /// Package AST.
    Ast,
    /// HIR elaboration / overload / evidence solving.
    Hir,
    /// Package semantic MIR + SPI (M1).
    SemanticMir,
    /// Cross-package link with selected std adaptors.
    LinkTime,
    /// Validated semantic MIR (M2).
    ValidateMir,
    /// Sparse representation / layout planning.
    RepresentationPlan,
    /// Target-private plan.
    TargetExecutionPlan,
    /// Artifact emit.
    Emit,
}

/// pipeline 阶段结构化诊断行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRecord {
    /// 稳定 machine code（如 `SMIR006`）。
    pub code: String,
    /// 严重级别字符串（`error` / `warning`）。
    pub severity: String,
    /// pipeline 阶段身份。
    pub stage: CompileStage,
    /// owning module / package symbol。
    pub module: String,
    /// 人类可读消息。
    pub message: String,
    /// 确定性排序 key。
    pub stable_sort_key: String,
}

/// pipeline 阶段诊断集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredDiagnosticSet {
    /// 诊断记录列表。
    pub records: Vec<DiagnosticRecord>,
}

impl StructuredDiagnosticSet {
    /// 从非空 records 构造；空 Vec 返回 None。
    pub fn from_records(records: Vec<DiagnosticRecord>) -> Option<Self> {
        if records.is_empty() { None } else { Some(Self { records }) }
    }
}

/// pipeline 阶段结果别名。
pub type StageResult<T> = Result<T, StructuredDiagnosticSet>;

/// Analysis / link stage: HIR elaboration consumed → linked program.
pub trait LinkStage {
    /// Produce a closed linked program or structured diagnostics.
    fn link(&self) -> StageResult<LinkedSemanticProgram>;
}

/// M2 validation stage.
pub trait ValidateMirStage {
    /// Validate linked program into canonical MIR success type.
    fn validate(&self, linked: &LinkedSemanticProgram) -> StageResult<ValkyrieMirProgram>;
}

/// Representation planning stage (sparse side tables only).
pub trait RepresentationPlanStage {
    /// Plan layouts without rewriting CFG or inventing semantics.
    fn plan(&self, program: &ValkyrieMirProgram) -> StageResult<RepresentationPlan>;
}

/// Documented stage order for maintainers / agents.
pub const STAGE_ORDER: &[CompileStage] = &[
    CompileStage::Ast,
    CompileStage::Hir,
    CompileStage::SemanticMir,
    CompileStage::LinkTime,
    CompileStage::ValidateMir,
    CompileStage::RepresentationPlan,
    CompileStage::TargetExecutionPlan,
    CompileStage::Emit,
];
