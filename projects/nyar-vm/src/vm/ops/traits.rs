use crate::vm::core::NyarVM;
use crate::vm::VmError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_get_witness_table(
        &mut self,
        _t_idx: u16,
        _i_idx: u16,
        _module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_witness_method(&mut self, _idx: u16) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_open_existential(&mut self) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_close_existential(&mut self) -> Result<Option<usize>, VmError> {
        Ok(None)
    }
}
