//! `.odin` 执行字节码解码错误类型。

use std::fmt::{Display, Formatter};

/// `.odin` decode error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OdinDecodeError {
    /// 输入缓冲区长度不足。
    TooShort,
    /// magic 值非法。
    InvalidMagic(u32),
    /// 输入带 `.nyar` 程序集 magic：这是旧执行产物或被误当执行字节码的程序集，必须重建。
    LegacyNyarMagic,
    /// 模块格式版本与当前 `Odin_VERSION` 不一致；旧产物一律拒绝，不做兼容读取。
    InvalidVersion(u32),
    /// 常量种类字节未知。
    InvalidConstantKind(u8),
    /// 导入种类字节未知。
    InvalidImportKind(u8),
    /// 导出种类字节未知。
    InvalidExportKind(u8),
    /// section 种类字节未知。
    InvalidSectionKind(u8),
    /// 模块内 UTF-8 字符串解码失败。
    InvalidUtf8,
    /// `CallIntrinsic` 的 `operand1` 不是已知的 OVM intrinsic 指令编号。
    UnknownIntrinsicOpcode(i32),
}

impl Display for OdinDecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, ".odin buffer too short"),
            Self::InvalidMagic(magic) => write!(f, "invalid .odin magic: 0x{magic:08X} (expected 0x{:08X})", crate::ODIN_MAGIC),
            Self::LegacyNyarMagic => write!(
                f,
                "legacy `.nyar` execution artifact rejected: `.odin` is a distinct artifact identity; rebuild it as `.odin`"
            ),
            Self::InvalidVersion(version) => {
                write!(
                    f,
                    "unsupported .odin format version: found {version}, expected {} (rebuild the artifact)",
                    crate::Odin_VERSION
                )
            }
            Self::InvalidConstantKind(kind) => write!(f, "invalid constant kind: {kind}"),
            Self::InvalidImportKind(kind) => write!(f, "invalid import kind: {kind}"),
            Self::InvalidExportKind(kind) => write!(f, "invalid export kind: {kind}"),
            Self::InvalidSectionKind(kind) => write!(f, "invalid section kind: {kind}"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in .odin module"),
            Self::UnknownIntrinsicOpcode(value) => write!(f, "unknown intrinsic opcode: {value}"),
        }
    }
}

impl std::error::Error for OdinDecodeError {}
