//! `CLR` 二进制后端。
//!
//! 直接写入 `PE/COFF` 二进制，不依赖 `ilasm`，保证跨平台可用。
//! `MSIL / PE / COFF` 格式模型经 [`crate::transitional`] 过渡，待迁入 Acorn。

#![deny(missing_docs)]

pub mod hosting;
pub mod host_process;
mod msil_text_writer;

pub use crate::transitional::{
    binary::{
        coff::{CoffHeader, CoffMachine, CoffObject, CoffRelocation, CoffRelocationKind, CoffSection, CoffSymbol},
        pe::{ClrMetadataBuilder, ClrMetadataError, PeWriter, PeWriterError, PeWriterOptions},
    },
    msil::{
        MethodBodyEncoder, MethodBodyError, MsilAssembly, MsilInstruction, MsilInstructionOperand, MsilMethodBody, MsilMethodRef,
        MsilMethodSignature, MsilModule, MsilOpcode, MsilParser, MsilTextMethod, MsilType, MsilTypeDef,
    },
};
pub use hosting::{DotNetFramework, DotNetRuntimeConfig, DotNetRuntimeOptions, write_dotnet_deps_json, write_dotnet_runtime_config};
pub use msil_text_writer::MsilTextWriter;

use std::path::PathBuf;

use crate::{DriverRunContract, EmitterError, ManagedAbiContract, Result};
use nyar::{
    abstractions::{ArtifactFormat, BackendInputKind},
    backends::{BackendDescriptor, CompilationOptions, TargetCodeGenBackend, clr::ClrImageKind},
    packaging::{ArtifactDescriptor, ArtifactSet, TargetLane},
};
/// `CLR` 二进制后端输入。
#[derive(Debug, Clone)]
pub struct ClrBinaryBackendInput {
    /// `MSIL` 模块。
    pub module: MsilModule,
    /// 输出目录。
    pub output_dir: PathBuf,
    /// 期望生成的镜像口味；为空时自动推断。
    pub image_kind: Option<ClrImageKind>,
    /// 导出入口的 managed ABI 契约（若有）。
    pub managed_abi: Option<ManagedAbiContract>,
    /// 后端已冻结的运行契约；驱动层按契约原样上报。
    pub run_contracts: Vec<DriverRunContract>,
}

/// `CLR` 二进制后端。
///
/// 将 `MsilModule` 直接编码为 `PE/COFF` 二进制 `.exe`，
/// 不调用 `ilasm`，保证 `Linux` 上也可运行（生成产物供 `Windows` 运行）。
pub struct ClrBinaryBackend {
    /// 后端描述。
    descriptor: BackendDescriptor,
}

impl ClrBinaryBackend {
    /// 创建一个新的 `CLR` 二进制后端。
    pub fn new() -> Self {
        Self {
            descriptor: BackendDescriptor {
                name: "clr-binary".to_string(),
                input_kind: BackendInputKind::MsilText,
                supported_targets: Vec::new(),
            },
        }
    }
}

impl Default for ClrBinaryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ClrBinaryBackend {
    fn validate_artifact(&self, input: &ClrBinaryBackendInput) -> Result<()> {
        if input.module.global_methods.is_empty() {
            return Err("CLR 后端至少需要一个全局方法".to_owned());
        }
        if image_kind(input) == ClrImageKind::Executable {
            let entry = input.module.global_methods.iter().find(|method| method.is_entry_point).ok_or_else(|| {
                "CLR 可执行镜像缺少 `is_entry_point` 方法：拒绝写出无法启动的 PE".to_owned()
            })?;
            validate_clr_entry_signature(entry)?;
        }
        Ok(())
    }

    fn compile_artifact(&self, input: ClrBinaryBackendInput, options: &CompilationOptions) -> Result<ArtifactSet> {
        std::fs::create_dir_all(&input.output_dir).map_err(|e| format!("创建输出目录失败：{}: {e}", input.output_dir.display()))?;
        let image_kind = image_kind(&input);
        // 任何带托管入口点、将由 `dotnet` 启动的镜像（可执行 `.exe` 或带合成入口的
        // `.dll`）都需要同目录的 runtimeconfig；框架依赖镜像缺它不是可选项。
        let has_entry_point = input.module.global_methods.iter().any(|method| method.is_entry_point);
        let library_with_entry_point = image_kind == ClrImageKind::DynamicLibrary && has_entry_point;
        let pe_image_kind = match image_kind {
            ClrImageKind::Executable => crate::transitional::binary::pe::ClrImageKind::Executable,
            // `acorn-pe` 只为 `Executable` flavor 写 CLI `EntryPointToken`；带合成入口的
            // 库镜像必须借该 flavor 才能被 `dotnet exec` 启动，产物文件名仍保持 `.dll`。
            ClrImageKind::DynamicLibrary if library_with_entry_point => crate::transitional::binary::pe::ClrImageKind::Executable,
            ClrImageKind::DynamicLibrary => crate::transitional::binary::pe::ClrImageKind::DynamicLibrary,
        };
        let writer = PeWriter::new(PeWriterOptions {
            assembly_name: options.artifact_name.clone(),
            module_name: options.artifact_name.clone(),
            image_kind: pe_image_kind,
        });
        // 程序集身份是镜像属性，只由产物名决定。降层期写入的模块名只是占位，
        // 这里统一覆盖，避免 Assembly 表与 `PeWriterOptions` 出现两个不同身份。
        let mut module = input.module;
        module.assembly.name = options.artifact_name.clone();
        let bytes = writer.write_module(&module).map_err(|error| format!("CLR PE 编码失败：{error}"))?;
        let relative = format!("{}.{}", options.artifact_name, image_kind.file_extension());
        let artifact_path = input.output_dir.join(&relative);
        std::fs::write(&artifact_path, bytes).map_err(|e| format!("写入 CLR 产物失败：{}: {e}", artifact_path.display()))?;

        if has_entry_point {
            write_dotnet_runtime_config(&input.output_dir, &options.artifact_name)?;
        }

        let mut artifacts = ArtifactSet::default();
        artifacts.push(ArtifactDescriptor {
            name: options.artifact_name.clone(),
            path: relative,
            kind: image_kind.artifact_kind(),
            format: ArtifactFormat::RawBinary,
            target: options.target.clone(),
            lane: TargetLane::Clr,
        });
        Ok(artifacts)
    }
}

impl TargetCodeGenBackend for ClrBinaryBackend {
    type Input = ClrBinaryBackendInput;

    fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    fn validate(&self, input: &Self::Input) -> nyar::backends::BackendResult<()> {
        self.validate_artifact(input).map_err(|error| nyar::backends::BackendError::for_backend(&self.descriptor.name, error))
    }

    fn compile(&self, input: Self::Input, options: &CompilationOptions) -> nyar::backends::BackendResult<ArtifactSet> {
        self.compile_artifact(input, options)
            .map_err(|error| nyar::backends::BackendError::for_backend(&self.descriptor.name, error))
    }
}

/// 输入未显式指定镜像口味时，按是否存在入口点推断。
fn image_kind(input: &ClrBinaryBackendInput) -> ClrImageKind {
    input.image_kind.unwrap_or_else(|| ClrImageKind::infer(input.module.global_methods.iter().any(|method| method.is_entry_point)))
}

/// CLR 入口点只接受 `void`/`int32` 返回、无参或 `string[]` 形参的静态方法。
fn validate_clr_entry_signature(entry: &MsilMethodBody) -> Result<()> {
    let return_ok = matches!(entry.method.signature.return_type, MsilType::Void | MsilType::Int32 { signed: true });
    if !return_ok {
        return Err(format!(
            "CLR 入口方法 `{}` 返回类型 `{}` 不符合 CLR entry 合同（只接受 `void`/`int32`）",
            entry.method.name, entry.method.signature.return_type
        ));
    }
    let parameters_ok = match entry.method.signature.parameter_types.as_slice() {
        [] => true,
        [MsilType::SzArray(element)] => matches!(element.as_ref(), MsilType::String),
        _ => false,
    };
    if !parameters_ok {
        return Err(format!(
            "CLR 入口方法 `{}` 形参 `{}` 不符合 CLR entry 合同（只接受无参或 `string[]`）",
            entry.method.name, entry.method.signature.parameter_list_text()
        ));
    }
    Ok(())
}
