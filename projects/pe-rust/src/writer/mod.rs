use crate::{
    PeContext,
    exports::nyar::pe_assembly::writer::{Guest, PeError, PeProgram, WriterConfig},
};

impl Guest for PeContext {
    fn write(program: PeProgram, config: WriterConfig) -> Result<Vec<u8>, PeError> {
        todo!()
    }

    fn validate(program: PeProgram) -> Result<bool, PeError> {
        todo!()
    }

    fn calculate_size(program: PeProgram) -> Result<u32, PeError> {
        todo!()
    }
}
