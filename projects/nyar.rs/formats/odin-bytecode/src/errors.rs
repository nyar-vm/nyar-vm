//! `.nyar` 解码错误类型。

use std::fmt::{Display, Formatter};

/// `.nyar` decode error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OdinDecodeError {
    /// 输入缓冲区长度不足。
    TooShort,
    /// magic 值非法。
    InvalidMagic(u32),
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
            Self::TooShort => write!(f, ".nyar buffer too short"),
            Self::InvalidMagic(magic) => write!(f, "invalid .nyar magic: 0x{magic:08X}"),
            Self::InvalidConstantKind(kind) => write!(f, "invalid constant kind: {kind}"),
            Self::InvalidImportKind(kind) => write!(f, "invalid import kind: {kind}"),
            Self::InvalidExportKind(kind) => write!(f, "invalid export kind: {kind}"),
            Self::InvalidSectionKind(kind) => write!(f, "invalid section kind: {kind}"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in .nyar module"),
            Self::UnknownIntrinsicOpcode(value) => write!(f, "unknown intrinsic opcode: {value}"),
        }
    }
}

impl std::error::Error for OdinDecodeError {}
