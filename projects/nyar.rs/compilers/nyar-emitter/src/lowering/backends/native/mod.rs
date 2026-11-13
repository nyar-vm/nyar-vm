//! Native 产物后端的 lowering 入口。
//!
//! `native` 不是平台，也不是编码身份：它只表示「产物是宿主可直接加载的原生镜像」
//! 这一形态（相对 JVM / CLR / WASM 托管产物）。具体机器码与容器由解析后的规范化
//! 目标四元组 [`CanonicalTarget`]（`arch-vendor-os-abi`）决定——`native`、`node`、
//! `python` 都必须先展开成四元组，再由本模块按架构分派到具体 ISA 后端。
//!
//! 当前只闭合 `x86_64-pc-windows-msvc`：x86-64 机器码 + `PE32+` 控制台可执行文件，
//! 入口以 `rax` 返回进程退出码。其余四元组（`x86_64-unknown-linux-gnu` 的 ELF、
//! aarch64、Mach-O 等）一律结构化失败，不借用其他平台的编码或容器。
//!
//! 算子编码同样按 ISA 冻结（[`x86_64::X86_64OperatorCode`]），不按「native」命名：
//! Node `.node` / Python `.pyd` 只是同一 ISA 上的宿主装载与 ABI 包装形态，不是独立
//! 指令集；宿主调用属于导入合同，不属于算子编码。

mod x64_lir;
pub mod x86_64;

use nyar::abstractions::{CanonicalAbi, CanonicalArch, CanonicalSpecification, CanonicalTarget};

use crate::{FragmentSubmission, Result};

/// 降低结果：完整原生镜像字节与入口位置事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLoweredFragment {
    /// 完整镜像字节（当前为 `PE32+`）。
    pub executable: Vec<u8>,
    /// 入口点在文本节内的偏移。
    pub entry_offset: u32,
    /// 文本节原始长度（未按文件对齐填充）。
    pub text_len: usize,
}

/// 按解析后的规范目标四元组降低为原生镜像。
///
/// 四元组是唯一分派依据；`TargetFamily::Native` 只说明产物形态，不能用来选编码、
/// 容器或 ABI。未实现的目标四元组必须失败闭合，指名具体四元组。
pub(crate) fn lower_fragment_to_native_image(submission: &FragmentSubmission, target: CanonicalTarget) -> Result<NativeLoweredFragment> {
    match target.arch {
        CanonicalArch::X86_64 => match target.specification {
            CanonicalSpecification::Windows => match target.abi {
                // x86-64 Windows 的 MSVC 与 GNU 目标共用 Win64 调用约定与 PE 容器。
                None | Some(CanonicalAbi::Msvc) | Some(CanonicalAbi::Gnu) => x86_64::lower_x86_64_windows_pe(submission),
                Some(other) => Err(format!(
                    "Native 后端不支持 `x86_64` + `windows` + `{}` 的 ABI：只闭合 Win64 调用约定（target={}）",
                    abi_name(other),
                    target.as_canonical_str()
                )),
            },
            other => Err(format!(
                "Native 后端没有 `x86_64` + `{}` 的容器实现：拒绝借用其他平台的镜像格式（target={}）",
                specification_name(other),
                target.as_canonical_str()
            )),
        },
        other => Err(format!("Native 后端尚未实现架构 `{}` 的机器码降低（target={}）", other.as_str(), target.as_canonical_str())),
    }
}

fn specification_name(specification: CanonicalSpecification) -> &'static str {
    match specification {
        CanonicalSpecification::Unknown => "unknown",
        CanonicalSpecification::Browser => "browser",
        CanonicalSpecification::Wasi => "wasi",
        CanonicalSpecification::Windows => "windows",
        CanonicalSpecification::Linux => "linux",
        CanonicalSpecification::MacOs => "macos",
        CanonicalSpecification::Android => "android",
        CanonicalSpecification::Ios => "ios",
    }
}

fn abi_name(abi: CanonicalAbi) -> &'static str {
    match abi {
        CanonicalAbi::Managed => "managed",
        CanonicalAbi::Clr => "clr",
        CanonicalAbi::Jvm => "jvm",
        CanonicalAbi::WebAssembly => "wasm",
        CanonicalAbi::Wasi => "wasi",
        CanonicalAbi::WasiP3 => "wasip3",
        CanonicalAbi::Msvc => "msvc",
        CanonicalAbi::Gnu => "gnu",
        CanonicalAbi::SystemV => "systemv",
        CanonicalAbi::Aapcs64 => "aapcs64",
    }
}
