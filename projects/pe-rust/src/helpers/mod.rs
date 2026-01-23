use crate::{
    PeContext,
    exports::nyar::pe_assembly::easy_test::{Guest, PeError, TargetArch},
};

impl Guest for PeContext {
    fn easy_exit_code(arch: TargetArch, code: u32) -> Result<Vec<u8>, PeError> {
        todo!()
    }

    fn easy_console_log(arch: TargetArch, text: String) -> Result<Vec<u8>, PeError> {
        todo!()
    }
}
