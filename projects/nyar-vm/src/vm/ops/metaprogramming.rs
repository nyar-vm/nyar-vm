use crate::vm::core::NyarVM;
use crate::vm::VmError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_quote(&mut self, _idx: u32) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_splice(&mut self) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_eval(&mut self, _argc: u8) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_expand_macro(
        &mut self,
        _idx: u16,
        _argc: u8,
        _module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        Ok(None)
    }
}
