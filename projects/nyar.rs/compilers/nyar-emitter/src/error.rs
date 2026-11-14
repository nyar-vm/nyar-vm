//! Emitter 操作错误（替代 miette `Report` / `miette!`）。

use std::fmt::{Display, Formatter};

/// `nyar-emitter` 内 `Result` 别名（与 `nyar::backends::BackendError` 同形）。
pub type EmitterError = String;

/// `nyar-emitter` 内 `Result` 别名。
pub type Result<T> = std::result::Result<T, EmitterError>;

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
