//! Compiler → emitter 管线边界错误（无 miette）。

use std::fmt::{Display, Formatter};

use crate::compiler::hir::lowering::HirError;
use nyar::PlanningError;

/// `compiler::emitter::pipeline` 内 `Result` 别名。
pub type PipelineResult<T> = std::result::Result<T, PipelineError>;

/// 编译到产物驱动链上的失败。
#[derive(Debug)]
pub enum PipelineError {
    /// HIR 降级失败。
    Hir(HirError),
    /// Nyar 规划失败。
    Planning(PlanningError),
    /// Emitter 后端失败。
    Emitter(nyar_emitter::EmitterError),
    /// 产物装配失败。
    Assembly(String),
    /// 产物表面校验失败。
    Surface(super::backend_bundle::ArtifactSurfaceError),
    /// 管线级一句说明。
    Message(String),
}

impl PipelineError {
    /// 构造 `Message` 变体。
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl Display for PipelineError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hir(error) => Display::fmt(error, f),
            Self::Planning(error) => Display::fmt(error, f),
            Self::Emitter(message) => f.write_str(message),
            Self::Assembly(message) => f.write_str(message),
            Self::Surface(error) => Display::fmt(error, f),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hir(error) => Some(error),
            Self::Planning(error) => Some(error),
            Self::Surface(error) => Some(error),
            Self::Emitter(_) | Self::Assembly(_) | Self::Message(_) => None,
        }
    }
}

impl From<HirError> for PipelineError {
    fn from(value: HirError) -> Self {
        Self::Hir(value)
    }
}

impl From<PlanningError> for PipelineError {
    fn from(value: PlanningError) -> Self {
        Self::Planning(value)
    }
}

impl From<nyar_emitter::EmitterError> for PipelineError {
    fn from(value: nyar_emitter::EmitterError) -> Self {
        Self::Emitter(value)
    }
}
