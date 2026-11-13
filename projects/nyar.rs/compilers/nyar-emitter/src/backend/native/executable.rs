use std::fmt;

use crate::transitional::binary::{
    elf::{Elf64ParseError, parse_elf64},
    pe::{Pe64ParseError, parse_pe64},
};
use nyar::abstractions::BinaryArch;

const MACHINE_AMD64: u16 = 0x8664;
const OPTIONAL_MAGIC_PE32_PLUS: u16 = 0x020B;
/// `IMAGE_FILE_DLL`：PE 头 characteristics 位，表示镜像是 DLL 而非可执行文件。
const IMAGE_FILE_DLL: u16 = 0x2000;
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
    /// 共享库镜像：Node `.node`、Python `.pyd`/`.so` 等宿主扩展载体。
    ElfShared,
}

/// Native 镜像的可解析事实：格式、架构与是否为共享库。
///
/// 「可解析」不等于「可降低/可运行」；调用方必须按各自能力边界消费这些事实，
/// 不得把架构或共享/可执行分类重新按文件名推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeImageFacts {
    /// 识别出的镜像格式。
    pub kind: NativeExecutableKind,
    /// 镜像机器架构。
    pub arch: BinaryArch,
    /// 是否为共享库镜像（DLL / `ET_DYN`）。
    pub shared: bool,
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
    classify_native_image(bytes).map(|facts| facts.kind)
}

/// 识别 native 镜像的格式、架构与共享/可执行分类。
///
/// 共享库是 Node `.node` / Python `.pyd`、`.so` 等宿主扩展的物理载体；
/// 这里只回答「可解析」事实，是否可降低/可运行由各能力边界决定。
pub fn classify_native_image(bytes: &[u8]) -> Result<NativeImageFacts, NativeExecutableError> {
    if bytes.len() < 4 {
        return Err(NativeExecutableError::TooShort);
    }
    let pe = parse_pe64(bytes);
    if let Ok(header) = &pe
        && header.machine == MACHINE_AMD64
        && header.optional_magic == OPTIONAL_MAGIC_PE32_PLUS
    {
        return Ok(NativeImageFacts { kind: NativeExecutableKind::Pe, arch: BinaryArch::X64, shared: pe_file_is_dll(bytes, header.pe_offset) });
    }
    let elf = parse_elf64(bytes);
    if let Ok(header) = &elf {
        match (header.file_type, header.machine) {
            (ET_EXEC, EM_X86_64) => {
                return Ok(NativeImageFacts { kind: NativeExecutableKind::Elf, arch: BinaryArch::X64, shared: false });
            }
            (ET_DYN, EM_X86_64) => {
                return Ok(NativeImageFacts { kind: NativeExecutableKind::ElfShared, arch: BinaryArch::X64, shared: true });
            }
            (ET_DYN, EM_AARCH64) => {
                return Ok(NativeImageFacts { kind: NativeExecutableKind::ElfShared, arch: BinaryArch::Arm64, shared: true });
            }
            _ => {}
        }
    }
    match (pe, elf) {
        // 解析成功但架构/文件类型不在支持集内：这是受支持的「可解析」事实，
        // 不是解析失败，必须返回结构化错误而不是对成功结果 `unwrap_err()` panic。
        (Ok(_), _) | (_, Ok(_)) => Err(NativeExecutableError::ParsedButUnsupported),
        (Err(pe), Err(elf)) => Err(NativeExecutableError::Unrecognized { pe, elf }),
    }
}

/// 读取 PE COFF 头 `Characteristics` 的 `IMAGE_FILE_DLL` 位。
///
/// `parse_pe64` 只暴露 `pe_offset/machine/optional_magic`，DLL 分类必须按
/// COFF 头布局读取；偏移越界时按非 DLL 处理（镜像本身已被解析器接受）。
fn pe_file_is_dll(bytes: &[u8], pe_offset: usize) -> bool {
    // PE signature(4 字节) + COFF header；Characteristics 位于 COFF 头偏移 18。
    let offset = pe_offset + 4 + 18;
    bytes.get(offset..offset + 2).and_then(|raw| raw.try_into().ok()).is_some_and(|raw| u16::from_le_bytes(raw) & IMAGE_FILE_DLL != 0)
}
