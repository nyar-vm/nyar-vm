#![doc = include_str!("readme.md")]
#![deny(missing_docs)]

mod executable;

use std::path::PathBuf;

use crate::{
    EmitterError, Result,
    transitional::binary::{
        coff::{CoffMachine, CoffObjectWriter, CoffSection, CoffSymbol, coff_object_from_sections},
        pe::extract_pe_section,
    },
};
use nyar::{
    abstractions::{ArtifactFormat, BackendInputKind, BinaryArch, BinaryFlavor, BinaryTarget, TargetFamily},
    backends::{BackendDescriptor, CompilationOptions, TargetCodeGenBackend},
    packaging::{ArtifactDescriptor, ArtifactSet, TargetLane},
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
                    BinaryTarget::new(TargetFamily::Native, BinaryArch::Arm64, BinaryFlavor::Native),
                    BinaryTarget::new(TargetFamily::Native, BinaryArch::X86, BinaryFlavor::Native),
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

impl TargetCodeGenBackend for NativeBinaryBackend {
    type Input = NativeBinaryBackendInput;

    fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    fn validate(&self, input: &Self::Input) -> Result<()> {
        let facts = classify_native_image(&input.executable).map_err(|error| format!("Native 后端 executable 格式无效: {error}"))?;
        if let Some(extension) = input.artifact_extension.as_deref() {
            validate_artifact_extension(extension, facts.shared)?;
        }
        Ok(())
    }

    fn compile(&self, input: Self::Input, options: &CompilationOptions) -> Result<ArtifactSet> {
        if options.target.family != TargetFamily::Native {
            return Err(format!("Native 后端只接受 `Native` 目标族，实际得到 {:?}", options.target.family));
        }
        let facts = classify_native_image(&input.executable).map_err(|error| format!("Native 后端 executable 格式无效: {error}"))?;
        // 产物架构必须与请求目标一致：包装层不得把 AMD64 镜像写成 Arm64 产物。
        if options.target.arch != facts.arch {
            return Err(format!("Native 后端镜像架构 {:?} 与请求目标架构 {:?} 不一致", facts.arch, options.target.arch));
        }
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
    // COFF sidecar 只属于可执行镜像；共享库由宿主直接加载，不伪造目标文件载体。
    if !facts.shared {
        let object_path = input.output_dir.join(format!("{}.obj", options.artifact_name));
        let object = coff_object_from_sections(
            CoffMachine::Amd64,
            vec![CoffSection {
                name: ".text".to_string(),
                data: extract_pe_section(&input.executable, b".text").unwrap_or_default(),
                relocations: Vec::new(),
                characteristics: 0x6000_0020,
            }],
            vec![CoffSymbol { name: input.entry_symbol.clone(), section_index: 1, value: 0, storage_class: 2 }],
        );
        let object_bytes = CoffObjectWriter::write(&object).map_err(|error| format!("`COFF` sidecar 编码失败：{error}"))?;
        std::fs::write(&object_path, object_bytes).map_err(|e| format!("写入 `COFF` sidecar 失败：{}: {e}", object_path.display()))?;
        artifacts.push(ArtifactDescriptor {
            name: options.artifact_name.clone(),
            path: object_path.file_name().and_then(|name| name.to_str()).ok_or_else(|| format!("COFF sidecar 文件名无效"))?.to_owned(),
            kind: nyar::ArtifactKind::Object,
            format: ArtifactFormat::Coff,
            target: options.target.clone(),
            lane: TargetLane::Native,
        });
    }
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
