//! Emitter 操作错误（替代 miette `Report` / `miette!`）。

/// `nyar-emitter` 内 `Result` 别名（与 `nyar::backends::BackendError` 同形）。
pub type EmitterError = String;

/// `nyar-emitter` 内 `Result` 别名。
pub type Result<T> = std::result::Result<T, EmitterError>;
