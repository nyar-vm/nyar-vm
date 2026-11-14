#![doc = include_str!("readme.md")]
#![deny(missing_docs)]

mod executable;

use std::path::PathBuf;

use crate::{EmitterError, Result};
use nyar::{
    abstractions::{
        ArtifactFormat, BackendInputKind, BinaryArch, BinaryFlavor, BinaryTarget, CanonicalAbi, CanonicalSpecification, TargetFamily,
    },
    backends::{BackendDescriptor, CompilationOptions, TargetCodeGenBackend},
    packaging::{ArtifactDescriptor, ArtifactSet, TargetLane},
};

pub use crate::lowering::backends::native::{
    NativeTextFunction, NativeTextInstruction, NativeTextProgramFunction, NativeTextProgramImage, encode_native_text_function,
    encode_native_text_program,
};
pub use executable::{NativeExecutableError, NativeExecutableKind, NativeImageFacts, classify_native_executable, classify_native_image};

/// `Native` 后端输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBinaryBackendInput {
    /// 已编码的可执行文件（`PE` 或 `ELF`）。
    pub executable: Vec<u8>,
    /// 输出目录。
    pub output_dir: PathBuf,
    /// 入口符号名。
    pub entry_symbol: String,
    /// 产物扩展名（含点），如 `.node`（Node 原生扩展）、`.pyd`（Windows Python 扩展）、
    /// `.so`（Linux Python/共享库）、`.dll`、`.exe`。`None` 时按可执行文件默认命名。
    ///
    /// 扩展名只决定产物文件身份与 `ArtifactKind`，不改变镜像字节或语言语义；
    /// 非共享镜像不得声明共享库扩展名。
    pub artifact_extension: Option<String>,
}

/// `Native` 二进制后端（Windows PE / Linux ELF）。
pub struct NativeBinaryBackend {
    descriptor: BackendDescriptor,
}

impl NativeBinaryBackend {
    /// 创建一个新的原生二进制后端。
    pub fn new() -> Self {
        Self {
            descriptor: BackendDescriptor {
                name: "native".to_string(),
                input_kind: BackendInputKind::CoffObject,
                supported_targets: vec![
                    BinaryTarget::new(TargetFamily::Native, BinaryArch::X64, BinaryFlavor::Native),
                    // Arm64 仅对 ELF 共享库可识别；PE ARM64 不在解析支持集内。
                    BinaryTarget::new(TargetFamily::Native, BinaryArch::Arm64, BinaryFlavor::Native),
                ],
            },
        }
    }
}

impl Default for NativeBinaryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeBinaryBackend {
    fn validate_artifact(&self, input: &NativeBinaryBackendInput) -> Result<()> {
        let facts = classify_native_image(&input.executable).map_err(|error| format!("Native 后端 executable 格式无效: {error}"))?;
        if let Some(extension) = input.artifact_extension.as_deref() {
            validate_artifact_extension(extension, facts.shared)?;
        }
        Ok(())
    }

    fn compile_artifact(&self, input: NativeBinaryBackendInput, options: &CompilationOptions) -> Result<ArtifactSet> {
        if options.target.family != TargetFamily::Native {
            return Err(format!("Native 后端只接受 `Native` 目标族，实际得到 {:?}", options.target.family));
        }
        let facts = classify_native_image(&input.executable).map_err(|error| format!("Native 后端 executable 格式无效: {error}"))?;
        // 产物架构必须与请求目标一致：包装层不得把 AMD64 镜像写成 Arm64 产物。
        if options.target.arch != facts.arch {
            return Err(format!("Native 后端镜像架构 {:?} 与请求目标架构 {:?} 不一致", facts.arch, options.target.arch));
        }
        // OS 规格与 ABI 是规范四元组的一部分：声明了就必须与镜像容器一致，
        // 不得只看 arch 让 PE 伪装成 Linux/GNU 产物。
        validate_target_image(facts, &options.target)?;
        if let Some(extension) = input.artifact_extension.as_deref() {
            validate_artifact_extension(extension, facts.shared)?;
        }
        std::fs::create_dir_all(&input.output_dir).map_err(|e| format!("创建输出目录失败：{}: {e}", input.output_dir.display()))?;

        match facts.kind {
            NativeExecutableKind::Elf | NativeExecutableKind::ElfShared => compile_elf(input, options, facts),
            NativeExecutableKind::Pe => compile_pe(input, options, facts),
        }
    }
}

impl TargetCodeGenBackend for NativeBinaryBackend {
    type Input = NativeBinaryBackendInput;

    fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    fn validate(&self, input: &Self::Input) -> nyar::backends::BackendResult<()> {
        self.validate_artifact(input).map_err(|error| nyar::backends::BackendError::for_backend(&self.descriptor.name, error))
    }

    fn compile(&self, input: Self::Input, options: &CompilationOptions) -> nyar::backends::BackendResult<ArtifactSet> {
        self.compile_artifact(input, options).map_err(|error| nyar::backends::BackendError::for_backend(&self.descriptor.name, error))
    }
}

/// 目标四元组的 OS 规格与 ABI 必须与实际镜像容器一致。
///
/// `None` 表示该后端声明未细化规格（粗粒度目标），此时只按镜像容器包装；
/// 一旦声明，PE 只能对应 Windows/MSVC，ELF 只能对应 Linux/GNU 或 System V。
fn validate_target_image(facts: NativeImageFacts, target: &BinaryTarget) -> Result<()> {
    let is_pe = facts.kind == NativeExecutableKind::Pe;
    let is_elf = matches!(facts.kind, NativeExecutableKind::Elf | NativeExecutableKind::ElfShared);
    if let Some(specification) = target.specification {
        let compatible = match specification {
            CanonicalSpecification::Windows => is_pe,
            CanonicalSpecification::Linux => is_elf,
            CanonicalSpecification::MacOs | CanonicalSpecification::Android | CanonicalSpecification::Ios => false,
            CanonicalSpecification::Unknown | CanonicalSpecification::Browser | CanonicalSpecification::Wasi => false,
        };
        if !compatible {
            return Err(format!("Native 后端镜像容器 {:?} 与目标规格 {:?} 不一致：不得把镜像写成其他平台的产物", facts.kind, specification));
        }
    }
    if let Some(abi) = target.abi {
        let compatible = match abi {
            CanonicalAbi::Msvc => is_pe,
            CanonicalAbi::Gnu | CanonicalAbi::SystemV => is_elf,
            CanonicalAbi::Aapcs64
            | CanonicalAbi::Managed
            | CanonicalAbi::Clr
            | CanonicalAbi::Jvm
            | CanonicalAbi::WebAssembly
            | CanonicalAbi::Wasi
            | CanonicalAbi::WasiP3 => false,
        };
        if !compatible {
            return Err(format!("Native 后端镜像容器 {:?} 与目标 ABI {:?} 不一致：不得把镜像写成其他 ABI 的产物", facts.kind, abi));
        }
    }
    Ok(())
}

/// 宿主扩展名与镜像共享/可执行分类必须一致。
///
/// `.node` / `.pyd` / `.so` / `.dll` / `.dylib` 是共享库载体；把可执行镜像写成
/// 这些扩展名会让宿主加载失败，必须在写出前结构化拒绝。
fn validate_artifact_extension(extension: &str, shared: bool) -> Result<()> {
    if !extension.starts_with('.') {
        return Err(format!("Native 产物扩展名 `{extension}` 必须以 `.` 开头"));
    }
    let shared_extension = matches!(extension, ".node" | ".pyd" | ".so" | ".dll" | ".dylib");
    match (shared_extension, shared) {
        (true, false) => Err(format!("Native 产物扩展名 `{extension}` 需要共享库镜像，实际镜像是可执行文件")),
        (false, true) => {
            Err(format!("Native 产物扩展名 `{extension}` 与服务端共享库镜像不符：共享库请使用 `.node`/`.pyd`/`.so`/`.dll`/`.dylib`"))
        }
        _ => Ok(()),
    }
}

fn output_file_name(options: &CompilationOptions, input: &NativeBinaryBackendInput, facts: NativeImageFacts) -> String {
    if let Some(extension) = input.artifact_extension.as_deref() {
        return format!("{}{extension}", options.artifact_name);
    }
    match facts.kind {
        NativeExecutableKind::ElfShared => format!("{}.so", options.artifact_name),
        NativeExecutableKind::Pe => format!("{}.exe", options.artifact_name),
        NativeExecutableKind::Elf => options.artifact_name.clone(),
    }
}

fn artifact_kind(facts: NativeImageFacts) -> nyar::ArtifactKind {
    if facts.shared { nyar::ArtifactKind::DynamicLibrary } else { nyar::ArtifactKind::Executable }
}

fn compile_pe(input: NativeBinaryBackendInput, options: &CompilationOptions, facts: NativeImageFacts) -> Result<ArtifactSet> {
    let file_name = output_file_name(options, &input, facts);
    let executable_path = input.output_dir.join(&file_name);
    std::fs::write(&executable_path, &input.executable).map_err(|e| format!("写入 PE 可执行文件失败：{}: {e}", executable_path.display()))?;

    let mut artifacts = ArtifactSet::default();
    artifacts.push(ArtifactDescriptor {
        name: options.artifact_name.clone(),
        path: file_name.clone(),
        kind: artifact_kind(facts),
        format: ArtifactFormat::Pe,
        target: options.target.clone(),
        lane: TargetLane::Native,
    });
    // 已链接镜像不携带原始 object 的完整节、符号与重定位事实。
    // 这里不再从 `.text` 反抽伪造 `.obj`：真实 COFF 目标文件必须由 object
    // emitter 产生节/符号/重定位后再链接，缺失时只发布镜像产物。
    Ok(artifacts)
}

fn compile_elf(input: NativeBinaryBackendInput, options: &CompilationOptions, facts: NativeImageFacts) -> Result<ArtifactSet> {
    let file_name = output_file_name(options, &input, facts);
    let executable_path = input.output_dir.join(&file_name);
    std::fs::write(&executable_path, &input.executable).map_err(|e| format!("写入 ELF 可执行文件失败：{}: {e}", executable_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&executable_path).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&executable_path, perms).map_err(|e| e.to_string())?;
    }

    let mut artifacts = ArtifactSet::default();
    artifacts.push(ArtifactDescriptor {
        name: options.artifact_name.clone(),
        path: file_name,
        kind: artifact_kind(facts),
        format: ArtifactFormat::Elf,
        target: options.target.clone(),
        lane: TargetLane::Native,
    });
    Ok(artifacts)
}
