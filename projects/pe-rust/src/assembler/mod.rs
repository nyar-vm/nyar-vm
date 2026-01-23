use crate::{
    PeContext,
    exports::nyar::pe_assembly::program::{Guest, *},
};

mod x64;
mod x86;

// 实现 helpers 接口
impl Guest for PeContext {
    fn new(arch: TargetArch, subsystem: SubsystemType) -> PeProgram {
        todo!()
    }

    fn new_console_app(_arch: TargetArch) -> PeProgram {
        todo!()
    }

    fn new_gui_app(_arch: TargetArch) -> PeProgram {
        todo!()
    }

    fn add_section(_program: PeProgram, _section: PeSection) -> Result<PeProgram, PeError> {
        todo!()
    }

    fn add_import_table(_program: PeProgram, _import_table: ImportTable) -> Result<PeProgram, PeError> {
        todo!()
    }

    fn set_entry_point(_program: PeProgram, _entry_point: u32) -> Result<PeProgram, PeError> {
        todo!()
    }
}