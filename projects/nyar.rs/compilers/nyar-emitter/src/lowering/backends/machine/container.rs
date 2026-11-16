//! 共享库容器装配（target-lane 私有服务）。
//!
//! `PE32+` DLL 与 `ELF64 ET_DYN` 的字节布局归 Acorn 格式 owner；本模块只按已冻结
//! 的平台身份选择容器，并把已编码文本程序的符号、导入、调用点与数据段重定位
//! 逐项转发。**exe、普通共享库与宿主扩展复用这一条装配服务**：语言侧（包括
//! LegendVM 残差产品）不得再维护第二套容器映射。
//!
//! 数据段重定位里的 `X64DataTarget::Function` 是适配器意图，不是容器输入：
//! 文本程序装配器必须在产出镜像前把它解析成 `.text` 偏移，容器不解析名字。

use acorn_pe::elf::{
    SharedElfDataReference, SharedElfDataRelocation, SharedElfDataTarget, SharedElfImage, SharedElfImportCall, SharedElfSpec, SharedElfWriter,
    SharedObjectExport,
};
use acorn_pe::pe::{NativeDataRelocation, NativeDataSection, NativeExport, NativePeImage, NativePeWriter};
use acorn_pe::pe::NativeDataTarget as PeDataTarget;

use super::{X64DataTarget, X64Platform, X64ProgramImage};
use crate::Result;

/// 把已编码文本程序写成共享库容器：Windows → `PE32+` DLL，Linux → `ELF64 ET_DYN`。
///
/// 镜像没有进程入口点；宿主通过导出表按平台调用约定调用导出符号，导入调用经
/// `.idata` 的 IAT 槽（Win64）或 GOT 槽（SysV64）解析。
pub fn compile_shared_container(platform: X64Platform, image: &X64ProgramImage) -> Result<Vec<u8>> {
    if platform.is_windows() { write_pe_library(image) } else { write_elf_shared(image) }
}

/// 写成 `PE32+` DLL：导出函数 ABI 产物。
fn write_pe_library(image: &X64ProgramImage) -> Result<Vec<u8>> {
    let exports = image.symbols.iter().map(|(name, offset)| NativeExport { name: name.clone(), offset: *offset }).collect();
    let relocations: Vec<NativeDataRelocation> = image
        .data_relocations
        .iter()
        .map(|(offset, target)| {
            let target = match target {
                X64DataTarget::Text(offset) => PeDataTarget::Text(*offset),
                X64DataTarget::Data(offset) => PeDataTarget::Data(*offset),
                X64DataTarget::Function(name) => {
                    return Err(format!("数据段重定位仍引用未解析的函数 `{name}`：编码器必须先冻结布局"));
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
    .map_err(|error| format!("PE 共享库映像编码失败：{error}"))
}

/// 写成 `ELF64 ET_DYN`：宿主 `dlopen` + `dlsym`。
fn write_elf_shared(image: &X64ProgramImage) -> Result<Vec<u8>> {
    let exports: Vec<SharedObjectExport> = image.symbols.iter().map(|(name, offset)| SharedObjectExport { name: name.clone(), text_offset: *offset }).collect();
    let calls: Vec<SharedElfImportCall> = image.elf_got_fixups.iter().map(|(field, import)| SharedElfImportCall { rel32_field: *field, import: *import }).collect();
    let data_relocations: Vec<SharedElfDataRelocation> = image
        .data_relocations
        .iter()
        .map(|(offset, target)| {
            let target = match target {
                X64DataTarget::Text(offset) => SharedElfDataTarget::Text(*offset),
                X64DataTarget::Data(offset) => SharedElfDataTarget::Data(*offset),
                X64DataTarget::Function(name) => {
                    return Err(format!("数据段重定位仍引用未解析的函数 `{name}`：编码器必须先冻结布局"));
                }
            };
            Ok(SharedElfDataRelocation { offset: *offset, target })
        })
        .collect::<Result<_>>()?;
    let data_references: Vec<SharedElfDataReference> =
        image.data_references.iter().map(|(field, data_offset)| SharedElfDataReference { rel32_field: *field, data_offset: *data_offset }).collect();
    let elf_image = SharedElfImage { text: image.text.clone(), rodata: Vec::new(), data: image.data.clone(), bss: Vec::new() };
    SharedElfWriter::write_x86_64_shared(&SharedElfSpec {
        image: &elf_image,
        exports: &exports,
        imports: &image.elf_imports,
        calls: &calls,
        data_relocations: &data_relocations,
        data_references: &data_references,
        needed: &image.host_dependencies,
        soname: None,
    })
    .map_err(|error| format!("ELF 共享库映像编码失败：{error}"))
}
