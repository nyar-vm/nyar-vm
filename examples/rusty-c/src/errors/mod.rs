use nyar_types::{NyarError, SourceLocation};
use oak_core::errors::OakError;
use std::fmt;

/// Mini C 错误类型
#[derive(Debug)]
pub struct CError {
    pub kind: Box<CErrorKind>,
    pub location: SourceLocation,
}

/// Mini C 错误种类
#[derive(Debug)]
pub enum CErrorKind {
    /// 语法错误
    Syntax {
        message: String,
        offset: usize,
    },
    /// 语义错误
    Semantic {
        message: String,
    },
    /// I/O 错误
    Io(std::io::Error),
    /// 其他错误
    Other(String),
}

impl CError {
    /// 创建一个新的 CError
    pub fn new(kind: CErrorKind, location: SourceLocation) -> Self {
        Self {
            kind: Box::new(kind),
            location,
        }
    }

    /// 获取 i18n key
    pub fn key(&self) -> &'static str {
        match *self.kind {
            CErrorKind::Syntax { .. } => "error.c.syntax",
            CErrorKind::Semantic { .. } => "error.c.semantic",
            CErrorKind::Io(_) => "error.c.io",
            CErrorKind::Other(_) => "error.c.other",
        }
    }
}

impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.kind {
            CErrorKind::Syntax { message, offset } => {
                write!(f, "Syntax error at offset {}: {}", offset, message)
            }
            CErrorKind::Semantic { message } => write!(f, "Semantic error: {}", message),
            CErrorKind::Io(e) => write!(f, "IO error: {}", e),
            CErrorKind::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for CError {}

impl From<OakError> for CError {
    fn from(e: OakError) -> Self {
        use oak_core::errors::OakErrorKind;
        let kind = match e.kind() {
            OakErrorKind::SyntaxError { message, offset, .. } => CErrorKind::Syntax {
                message: message.clone(),
                offset: *offset,
            },
            OakErrorKind::IoError { error, .. } => {
                // 这里我们只能克隆错误，因为 std::io::Error 不支持 Clone
                // 在实际应用中，可能需要更好的处理方式
                CErrorKind::Other(format!("IO Error: {}", error))
            }
            _ => CErrorKind::Other(format!("{:?}", e.kind())),
        };
        Self::new(kind, SourceLocation::default())
    }
}

impl From<CError> for NyarError {
    fn from(e: CError) -> Self {
        match *e.kind {
            CErrorKind::Syntax { message, .. } => NyarError::Parse(message),
            CErrorKind::Semantic { message } => NyarError::Compile(message),
            CErrorKind::Io(err) => NyarError::from(err),
            CErrorKind::Other(msg) => NyarError::Compile(msg),
        }
    }
}
