//! Wrap the native lane's encoded x64 program into a Windows `PE32+` DLL.
//!
//! This is the **container** step for residual products after the native target
//! lane has encoded the text LIR. exe images belong to the formal native lane
//! (`nyar-emitter` lowering + acorn-pe), not to this runtime; there is no
//! standalone exe exit here. Unrelated to OVM / `.odin`.

use acorn_pe::pe::{
    NativeDataRelocation, NativeDataSection, NativeDataTarget, NativeExport, NativePeImage, NativePeWriter,
};
use miette::Result;
use nyar_emitter::nyar_backend_native::{NativeDataTarget as ImageDataTarget, NativeTextProgramImage};

/// PE library compiler for the Legend VM native residual products.
#[derive(Debug, Default, Clone, Copy)]
pub struct PeCompiler;

impl PeCompiler {
    /// 把已编码的文本、导出符号与宿主导入写成 `PE32+` DLL（导出函数 ABI 产物）。
    ///
    /// 镜像没有入口点；宿主通过导出表按 Win64 调用约定调用残差函数，导入调用经
    /// `.idata` 的 IAT 槽解析。
    pub fn compile_library(&self, image: &NativeTextProgramImage) -> Result<Vec<u8>> {
        let exports = image.symbols.iter().map(|(name, offset)| NativeExport { name: name.clone(), offset: *offset }).collect();
        let relocations: Vec<NativeDataRelocation> = image
            .data_relocations
            .iter()
            .map(|(offset, target)| {
                let target = match target {
                    ImageDataTarget::Text(offset) => NativeDataTarget::Text(*offset),
                    ImageDataTarget::Data(offset) => NativeDataTarget::Data(*offset),
                    ImageDataTarget::Function(name) => {
                        return Err(miette::miette!("数据段重定位仍引用未解析的函数 `{name}`：编码器必须先冻结布局"));
                    }
                };
                Ok(NativeDataRelocation { offset: *offset, target })
            })
            .collect::<Result<_>>()?;
        let data = NativeDataSection { data: image.data.clone(), relocations, text_fixups: image.data_references.clone() };
        NativePeWriter::write_executable_with_data(
            &NativePeImage {
                text: image.text.clone(),
                rdata: Vec::new(),
                idata: image.idata.clone(),
                imports: image.imports.clone(),
                exports,
                text_rel32_fixups: image.text_rel32_fixups.clone(),
                is_dll: true,
                entry_point: 0,
            },
            &data,
        )
        .map_err(|error| miette::miette!("PE 共享库映像编码失败：{error}"))
    }
}
