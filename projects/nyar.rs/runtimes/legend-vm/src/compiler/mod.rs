//! Legend VM compilers.
//!
//! Host-script native product path: [`native_residual`] -> x64 / Windows `PE` or Linux `ELF`.

mod elf;
mod native_residual;
mod pe;

pub use elf::ElfCompiler;
pub use native_residual::{
    NativeHostModule, NativePlatform, NativeProductTarget, NativeResidualFunction, NativeResidualModule, NativeResidualOp,
    NativeResidualSink, ResidualValue, emit_native_elf_library, emit_native_node_addon, emit_native_pe,
    emit_native_pe_library, emit_native_python_extension, eval_native_residual,
    eval_native_residual_with_inputs, eval_native_residual_with_trace, lower_native_residual_to_program, lower_native_residual_to_x64,
    specialize_language,
};
pub use pe::PeCompiler;
