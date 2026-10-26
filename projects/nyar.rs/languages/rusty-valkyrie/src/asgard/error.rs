//! Asgard 命令 / 管线操作错误。

use std::fmt::{Display, Formatter};

use crate::legion::cmds::error::LegionError;

/// Asgard API 的 `Result` 别名。
pub type AsgardResult<T> = std::result::Result<T, AsgardError>;

/// Asgard 操作失败。
#[derive(Debug)]
pub enum AsgardError {
    /// Legion 层错误上抛。
    Legion(LegionError),
    /// 裸 I/O 错误。
    Io(std::io::Error),
    /// 命令级一句说明。
    Message(String),
}

impl AsgardError {
    /// 构造 `Message` 变体。
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl Display for AsgardError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Legion(error) => Display::fmt(error, f),
            Self::Io(error) => Display::fmt(error, f),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AsgardError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Legion(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Message(_) => None,
        }
    }
}

impl From<LegionError> for AsgardError {
    fn from(value: LegionError) -> Self {
        Self::Legion(value)
    }
}

impl From<std::io::Error> for AsgardError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<nyar_emitter::EmitterError> for AsgardError {
    fn from(value: nyar_emitter::EmitterError) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<crate::compiler::emitter::pipeline::PipelineError> for AsgardError {
    fn from(value: crate::compiler::emitter::pipeline::PipelineError) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<zip::result::ZipError> for AsgardError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::msg(value.to_string())
    }
}
