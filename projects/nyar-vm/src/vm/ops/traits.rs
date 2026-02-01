use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use nyar_types::NyarError;

pub trait UnaryOp {
    fn execute(&self, vm: &mut NyarVM) -> Result<(), NyarError>;
}

pub trait BinaryOp {
    fn execute(&self, vm: &mut NyarVM) -> Result<(), NyarError>;
}

impl NyarVM {
    #[inline(always)]
    pub fn execute_get_witness_table(
        &mut self,
        t_idx: u16,
        i_idx: u16,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        // Find the impl that matches t_idx and i_idx
        let module = &self.modules[module_idx];
        let impl_info = module
            .impls
            .iter()
            .find(|im| im.class_idx == t_idx && im.trait_idx == i_idx)
            .ok_or_else(|| {
                self.error(nyar_types::VmErrorKind::ImplNotFound {
                    class: t_idx,
                    trait_id: i_idx,
                })
            })?;

        let methods = impl_info.methods.clone();
        let witness = Value::witness_table(module_idx, methods, &self.gc);
        self.push(witness)?;

        Ok(None)
    }

    #[inline(always)]
    pub fn execute_witness_method(&mut self, _idx: u16) -> Result<Option<usize>, NyarError> {
        // Pop a witness table from the stack, get method at idx, push it.
        let witness_val = self.pop()?;
        let witness = unsafe { witness_val.as_witness_table() };
        let chunk_idx = witness
            .methods
            .get(_idx as usize)
            .ok_or_else(|| self.error(nyar_types::VmErrorKind::IndexOutOfBounds(_idx as usize)))?;

        // Push the method as a closure or some callable value
        let closure = Value::closure(witness.module_idx, *chunk_idx, vec![], &self.gc);
        self.push(closure)?;

        Ok(None)
    }

    #[inline(always)]
    pub fn execute_open_existential(&mut self) -> Result<Option<usize>, NyarError> {
        let trait_val = self.pop()?;
        let trait_obj = trait_val
            .try_as_trait_object()
            .ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x62)))?;

        let data = trait_obj.data;
        let witness = trait_obj.witness;

        // Pushing to stack (root) does not require write barriers
        self.push(data)?;
        self.push(witness)?;

        Ok(None)
    }

    #[inline(always)]
    pub fn execute_close_existential(&mut self) -> Result<Option<usize>, NyarError> {
        let witness = self.pop()?;
        let data = self.pop()?;

        let trait_obj = Value::trait_object(data, witness, &self.gc);
        self.push(trait_obj)?;

        Ok(None)
    }
}
