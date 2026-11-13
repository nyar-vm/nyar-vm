//! Emitter 操作错误（替代 miette `Report` / `miette!`）。

use std::fmt::{Display, Formatter};

/// `nyar-emitter` 内 `Result` 别名（与 `nyar::backends::BackendError` 同形）。
pub type EmitterError = String;

/// `nyar-emitter` 内 `Result` 别名。
pub type Result<T> = std::result::Result<T, EmitterError>;

/// `CompiledProgram` → 目标私有计划的公共结构化失败。
///
/// 已知的边界事实（片段身份、可达根身份）保留独立变体，调用方可以按类型分派；
/// 其余无法无损投影的语义原因保留在 [`Self::Unsound`] 的细节文本中。内部
/// 降层仍用文本原因，本类型是该公共边界唯一的错误面。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetPlanError {
    /// 片段身份不在 canonical 链接结果中。
    UnknownFragment {
        /// 请求的片段身份。
        fragment: String,
    },
    /// 可达根既不是 canonical 函数体，也不是已声明导入。
    UnknownRoot {
        /// 请求的根实例。
        instance: nyar_types::ItemInstanceId,
    },
    /// canonical 合同无法无损投影到目标计划。
    Unsound {
        /// 具体原因（来自内部降层）。
        reason: String,
    },
}

impl Display for TargetPlanError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFragment { fragment } => write!(f, "Canonical 片段 `{fragment}` 不存在"),
            Self::UnknownRoot { instance } => write!(f, "Compiler callable 实例 `{instance:?}` 缺少 canonical 函数体"),
            Self::Unsound { reason } => f.write_str(reason),
        }
    }
}

impl std::error::Error for TargetPlanError {}

impl From<String> for TargetPlanError {
    fn from(reason: String) -> Self {
        Self::Unsound { reason }
    }
}

/// 片段降低到驱动输入的公共结构化失败。
///
/// Semantic MIR 合同门禁有独立变体，调用方可以按类型分派；目标降低与装配
/// 的细节原因保留在 [`Self::Lowering`] 中。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragmentLoweringError {
    /// Semantic MIR 合同拒绝（保留 code / function / location / detail）。
    SemanticContract(crate::lowering::features::semantic_mir_contract::SemanticMirContractError),
    /// 目标降低或产物装配失败。
    Lowering {
        /// 具体原因。
        reason: String,
    },
}

impl Display for FragmentLoweringError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SemanticContract(error) => {
                write!(f, "semantic MIR contract failed [{}] {} at {}: {}", error.code, error.function, error.location, error.detail)
            }
            Self::Lowering { reason } => f.write_str(reason),
        }
    }
}

impl std::error::Error for FragmentLoweringError {}

impl From<String> for FragmentLoweringError {
    fn from(reason: String) -> Self {
        Self::Lowering { reason }
    }
}
