use std::fmt;

use crate::transitional::binary::{
    elf::{Elf64ParseError, parse_elf64},
    pe::{Pe64ParseError, parse_pe64},
};

const MACHINE_AMD64: u16 = 0x8664;
const OPTIONAL_MAGIC_PE32_PLUS: u16 = 0x020B;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

/// Native 后端可识别的可执行文件格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeExecutableKind {
    /// Windows `PE32+` / `AMD64`。
    Pe,
    /// Linux `ELF64 ET_EXEC` / `x86-64`。
    Elf,
    /// Android `ELF64 ET_DYN` / `AArch64` 共享库。
    ElfShared,
}

/// Native 后端可执行文件分类错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeExecutableError {
    /// 镜像过短。
    TooShort,
    /// 镜像可解析为 `PE`/`ELF`，但机器类型或文件类型不受本后端支持。
    ParsedButUnsupported,
    /// 既不是本后端支持的 `PE` 也不是 `ELF`。
    Unrecognized {
        /// `PE` 解析/约束失败原因。
        pe: Pe64ParseError,
        /// `ELF` 解析/约束失败原因。
        elf: Elf64ParseError,
    },
}

impl fmt::Display for NativeExecutableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "native executable 过短"),
            Self::ParsedButUnsupported => write!(f, "native executable 可解析但机器类型或文件类型不受支持"),
            Self::Unrecognized { pe, elf } => {
                write!(f, "不是受支持的 native PE ({pe}) 或 ELF ({elf})")
            }
        }
    }
}

impl std::error::Error for NativeExecutableError {}

/// 识别 native 后端可消费的可执行文件格式。
pub fn classify_native_executable(bytes: &[u8]) -> Result<NativeExecutableKind, NativeExecutableError> {
    if bytes.len() < 4 {
        return Err(NativeExecutableError::TooShort);
    }
    if is_supported_pe(bytes) {
        return Ok(NativeExecutableKind::Pe);
    }
    if is_supported_elf_shared(bytes) {
        return Ok(NativeExecutableKind::ElfShared);
    }
    if is_supported_elf(bytes) {
        return Ok(NativeExecutableKind::Elf);
    }
    match (parse_pe64(bytes), parse_elf64(bytes)) {
        // 解析成功但架构/文件类型不在支持集内：这是受支持的「可解析」事实，
        // 不是解析失败，必须返回结构化错误而不是对成功结果 `unwrap_err()` panic。
        (Ok(_), _) | (_, Ok(_)) => Err(NativeExecutableError::ParsedButUnsupported),
        (Err(pe), Err(elf)) => Err(NativeExecutableError::Unrecognized { pe, elf }),
    }
}

fn is_supported_pe(bytes: &[u8]) -> bool {
    match parse_pe64(bytes) {
        Ok(header) => header.machine == MACHINE_AMD64 && header.optional_magic == OPTIONAL_MAGIC_PE32_PLUS,
        Err(_) => false,
    }
}

fn is_supported_elf(bytes: &[u8]) -> bool {
    match parse_elf64(bytes) {
        Ok(header) => header.file_type == ET_EXEC && header.machine == EM_X86_64,
        Err(_) => false,
    }
}

fn is_supported_elf_shared(bytes: &[u8]) -> bool {
    match parse_elf64(bytes) {
        Ok(header) => header.file_type == ET_DYN && header.machine == EM_AARCH64,
        Err(_) => false,
    }
}
