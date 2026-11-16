//! Legend VM compilers.
//!
//! Host-script native product path: [`machine_residual`] -> x64 / Windows `PE` or Linux `ELF`.

mod machine_residual;

pub use machine_residual::{
    HostModule, X64Platform, X64ProductTarget, MachineResidualFunction, MachineResidualModule, MachineResidualOp, MachineResidualSink,
    ResidualValue, emit_machine_elf_library, emit_machine_node_addon, emit_machine_pe_library, emit_machine_python_extension, eval_machine_residual,
    eval_machine_residual_with_inputs, eval_machine_residual_with_trace, lower_machine_residual_to_program, specialize_language,
};
