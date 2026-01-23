use crate::{
    PeContext,
    exports::nyar::pe_assembly::x86_instructions::{EncodedInstruction, Guest, PeError, X86Instruction},
};

impl Guest for PeContext {
    fn encode(instruction: X86Instruction) -> Result<EncodedInstruction, PeError> {
        todo!()
    }

    fn decode(data: Vec<u8>) -> Result<X86Instruction, PeError> {
        todo!()
    }
}
