//! Wrap x64 machine code into a Windows PE executable.
//!
//! This is the **native product** packaging step after Futamura specialization
//! lowers a [`super::NativeResidualModule`] to x64 (subset today). It is unrelated
//! to OVM / `.odin`.

use std::{fs, path::Path};

use acorn_pe::pe::{NativeExport, NativePeImage, NativePeWriter};
use miette::{IntoDiagnostic, Result, WrapErr};
use nyar_emitter::nyar_backend_native::NativeTextProgramImage;

/// `ret`，使入口以 `rax` 作为进程退出码返回。
const RET: u8 = 0xC3;

/// `xor eax, eax; ret`，退出码 0。
const EMPTY_EPILOG: [u8; 3] = [0x31, 0xC0, RET];

/// PE executable compiler for the Legend VM native output.
#[derive(Debug, Default, Clone, Copy)]
pub struct PeCompiler;

impl PeCompiler {
    /// Compile x64 machine code to a PE file at `output_path`.
    pub fn compile(&self, _entry_function_name: &str, x64_code: &[u8], output_path: &Path) -> Result<Vec<u8>> {
        let bytes = self.compile_to_bytes(x64_code)?;
        if let Some(parent) = output_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("failed to create output directory: {}", parent.display()))?;
            }
        }
        fs::write(output_path, &bytes).into_diagnostic().wrap_err_with(|| format!("failed to write PE file: {}", output_path.display()))?;
        Ok(bytes)
    }

    /// Compile x64 machine code to PE bytes.
    pub fn compile_to_bytes(&self, x64_code: &[u8]) -> Result<Vec<u8>> {
        let mut text = Vec::with_capacity(x64_code.len() + 1);
        if x64_code.is_empty() {
            text.extend_from_slice(&EMPTY_EPILOG);
        }
        else {
            text.extend_from_slice(x64_code);
            text.push(RET);
        }

        NativePeWriter::write_executable(&NativePeImage {
            text,
            rdata: Vec::new(),
            idata: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            text_rel32_fixups: Vec::new(),
            is_dll: false,
            entry_point: 0,
        })
        .map_err(|error| miette::miette!("PE 可执行映像编码失败：{error}"))
    }

    /// 把已编码的文本、导出符号与宿主导入写成 `PE32+` DLL（导出函数 ABI 产物）。
    ///
    /// 镜像没有入口点；宿主通过导出表按 Win64 调用约定调用残差函数，导入调用经
    /// `.idata` 的 IAT 槽解析。
    pub fn compile_library(&self, image: &NativeTextProgramImage) -> Result<Vec<u8>> {
        let exports = image.symbols.iter().map(|(name, offset)| NativeExport { name: name.clone(), offset: *offset }).collect();
        NativePeWriter::write_executable(&NativePeImage {
            text: image.text.clone(),
            rdata: Vec::new(),
            idata: image.idata.clone(),
            imports: image.imports.clone(),
            exports,
            text_rel32_fixups: image.text_rel32_fixups.clone(),
            is_dll: true,
            entry_point: 0,
        })
        .map_err(|error| miette::miette!("PE 共享库映像编码失败：{error}"))
    }
}
