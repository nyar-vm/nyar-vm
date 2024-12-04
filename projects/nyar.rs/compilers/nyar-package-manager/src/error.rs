use miette::Diagnostic;
use nyar_package_registry::RegistryError;
use oak_core::OakError;
use thiserror::Error;

/// 包管理器错误类型。
#[derive(Debug, Error, Diagnostic)]
pub enum PackageManagerError {
    /// 注册表适配器返回的 HTTP / 协议错误。
    #[error(transparent)]
    #[diagnostic(code(nyar::package_manager::registry))]
    Registry(#[from] RegistryError),
    /// VON 解析或序列化失败。
    #[error(transparent)]
    #[diagnostic(code(nyar::package_manager::von))]
    Von(#[from] OakError),
    /// 本地文件系统读写失败。
    #[error(transparent)]
    #[diagnostic(code(nyar::package_manager::io))]
    Io(#[from] std::io::Error),
    /// 带用户可读说明的一般性失败。
    #[error("{0}")]
    #[diagnostic(code(nyar::package_manager::message))]
    Message(String),
}

impl PackageManagerError {
    /// 构造仅包含说明文本的错误。
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

/// 包管理器操作的 `Result` 别名。
pub type Result<T> = std::result::Result<T, PackageManagerError>;
