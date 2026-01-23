use crate::{
    PeContext,
    exports::nyar::pe_assembly::x64_instructions::{EncodedInstruction, Guest, PeError, X64Instruction},
};

impl Guest for PeContext {
    fn encode(instruction: X64Instruction) -> Result<EncodedInstruction, PeError> {
        todo!()
    }

    fn decode(data: Vec<u8>) -> Result<X64Instruction, PeError> {
        todo!()
    }
}
