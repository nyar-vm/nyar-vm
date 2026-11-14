//! Legend VM compilers.
//!
//! Host-script native product path: [`native_residual`] -> x64 / Windows `PE` or Linux `ELF`.
//! [`residual::StackResidualSink`] / [`bytecode`] are probe-only (`.odin`).

mod bytecode;
mod elf;
mod generate;
mod native_residual;
mod pe;
mod residual;
mod stack;

pub use bytecode::NyarBytecodeCompiler;
pub use elf::ElfCompiler;
pub use generate::GenerateModule;
pub use native_residual::{
    NativeProductTarget, NativeResidualFunction, NativeResidualModule, NativeResidualOp, NativeResidualSink, NativeResidualTarget,
    ResidualValue, emit_native_elf_library, emit_native_node_addon, emit_native_pe, emit_native_pe_library,
    emit_native_python_extension, eval_native_residual,
    eval_native_residual_with_inputs, eval_native_residual_with_trace, lower_native_residual_to_program, lower_native_residual_to_x64,
    specialize_language,
};
pub use pe::PeCompiler;
pub use residual::StackResidualSink;
pub use stack::StackCompiler;

/// Artifact from [`crate::LegendVmRunner::compile_module`].
#[derive(Debug, Clone, PartialEq)]
pub enum CompileArtifact {
    /// Host-script Futamura product: native-bound residual (-> PE).
    Native(NativeResidualModule),
    /// Bytecode-pipe probe only (JS/Python stubs) — **not** host-script PE destination.
    BytecodeProbe(GenerateModule),
}
