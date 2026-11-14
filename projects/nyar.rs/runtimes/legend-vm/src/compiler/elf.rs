//! Wrap x86-64 machine code into a Linux `ELF` shared library.
//!
//! This is the Linux counterpart of [`super::PeCompiler::compile_library`]: the
//! native product is packaged as `ET_DYN`, and the host loads it through
//! `dlopen` + `dlsym`. Container bytes, the dynamic table, the writable data
//! segment and the `GOT` belong to the Acorn format owner; this module only
//! forwards the encoded symbols, imports, call sites and data relocations.

use acorn_pe::elf::{
    SharedElfDataReference, SharedElfDataRelocation, SharedElfDataTarget, SharedElfImage, SharedElfImportCall, SharedElfWriter,
    SharedObjectExport,
};
use miette::{Result, miette};
use nyar_emitter::nyar_backend_native::{NativeDataTarget, NativeTextProgramImage};

/// x86-64 `ELF` shared-library compiler for the Legend VM native output.
#[derive(Debug, Default, Clone, Copy)]
pub struct ElfCompiler;

impl ElfCompiler {
    /// Package an encoded text program (symbols + bytes) as an `ET_DYN` `.so`.
    pub fn compile_library(&self, image: &NativeTextProgramImage) -> Result<Vec<u8>> {
        let exports: Vec<SharedObjectExport> =
            image.symbols.iter().map(|(name, offset)| SharedObjectExport { name: name.clone(), text_offset: *offset }).collect();
        let calls: Vec<SharedElfImportCall> =
            image.elf_got_fixups.iter().map(|(field, import)| SharedElfImportCall { rel32_field: *field, import: *import }).collect();
        let data_relocations: Vec<SharedElfDataRelocation> = image
            .data_relocations
            .iter()
            .map(|(offset, target)| SharedElfDataRelocation {
                offset: *offset,
                target: match target {
                    NativeDataTarget::Text(offset) => SharedElfDataTarget::Text(*offset),
                    NativeDataTarget::Data(offset) => SharedElfDataTarget::Data(*offset),
                },
            })
            .collect();
        let data_references: Vec<SharedElfDataReference> = image
            .data_references
            .iter()
            .map(|(field, data_offset)| SharedElfDataReference { rel32_field: *field, data_offset: *data_offset })
            .collect();
        SharedElfWriter::write_x86_64_with_imports(
            &SharedElfImage { text: image.text.clone(), rodata: Vec::new(), data: image.data.clone(), bss: Vec::new() },
            &exports,
            &image.elf_imports,
            &calls,
            &data_relocations,
            &data_references,
        )
        .map_err(|error| miette!("ELF 共享库映像编码失败：{error}"))
    }
}
