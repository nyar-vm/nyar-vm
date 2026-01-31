use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;
use crate::bytecode::format::Constant;
use crate::vm::effects::perform_effect_internal;

impl NyarVM {
    #[inline(always)]
    pub fn execute_perform(
        &mut self,
        idx: u16,
        argc: u8,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        let name = match self.modules[module_idx].constants.get(idx as usize) {
            Some(Constant::String(s)) => s.clone(),
            _ => return Err(VmError::IndexOutOfBounds),
        };

        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let result = perform_effect_internal(self, module_idx, name, args)?;
        if let Some(val) = result {
            self.push(val)?;
        } else {
            self.push(Value::null())?;
        }
        
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_with_handler(
        &mut self,
        _idx: u16,
        _module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        // Placeholder for user-defined effect handlers
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_resume_with(&mut self) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_capture_cont(&mut self) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_await(&mut self) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_block_on(&mut self) -> Result<Option<usize>, VmError> {
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_match_effect(&mut self, _idx: u16) -> Result<Option<usize>, VmError> {
        Ok(None)
    }
}
