//! Wrap x86-64 machine code into a Linux `ELF` shared library.
//!
//! This is the Linux counterpart of [`super::PeCompiler::compile_library`]: the
//! native product is packaged as `ET_DYN`, and the host loads it through
//! `dlopen` + `dlsym`. Container bytes and the dynamic table belong to the
//! Acorn format owner; this module only forwards the encoded symbols.

use acorn_pe::elf::{SharedElfImage, SharedElfWriter, SharedObjectExport};
use miette::{Result, miette};
use nyar_emitter::nyar_backend_native::NativeTextProgramImage;

/// x86-64 `ELF` shared-library compiler for the Legend VM native output.
#[derive(Debug, Default, Clone, Copy)]
pub struct ElfCompiler;

impl ElfCompiler {
    /// Package an encoded text program (symbols + bytes) as an `ET_DYN` `.so`.
    pub fn compile_library(&self, image: &NativeTextProgramImage) -> Result<Vec<u8>> {
        let exports: Vec<SharedObjectExport> =
            image.symbols.iter().map(|(name, offset)| SharedObjectExport { name: name.clone(), text_offset: *offset }).collect();
        SharedElfWriter::write_x86_64(
            &SharedElfImage { text: image.text.clone(), rodata: Vec::new(), bss: Vec::new() },
            &exports,
        )
        .map_err(|error| miette!("ELF 共享库映像编码失败：{error}"))
    }
}
