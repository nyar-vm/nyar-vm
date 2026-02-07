use crate::bytecode::format::Constant;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::NyarError;
use nyar_types::QualifiedName;

impl NyarVM {
    #[inline(always)]
    pub fn execute_push(&mut self, idx: u16, module_idx: usize) -> Result<Option<usize>, NyarError> {
        let c = self.modules[module_idx]
            .constants
            .get(idx as usize)
            .ok_or_else(|| self.error(nyar_types::VmErrorKind::IndexOutOfBounds(idx as usize)))?;
        match c {
            Constant::Int(i) => self.push(Value::int(i.clone())),
            Constant::Float(x) => self.push(Value::float(x.clone())),
            Constant::String(s) => self.push(Value::string(s.clone(), &self.gc)),
            Constant::QualifiedName(qn) => {
                self.push(Value::qualified_name(qn.clone(), &self.gc))
            }
        }?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_pop_stack(&mut self) -> Result<Option<usize>, NyarError> {
        let _ = self.pop()?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_dup(&mut self, d: u16) -> Result<Option<usize>, NyarError> {
        let v = self.peek_at(d as usize)?;
        self.push(v)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_swap(&mut self, d: u16) -> Result<Option<usize>, NyarError> {
        let sp = self.sp;
        if sp == 0 {
            return Err(self.error(nyar_types::VmErrorKind::StackUnderflow));
        }
        let eff = if (d as usize) >= sp {
            sp - 1
        } else {
            d as usize
        };
        self.swap_with(eff)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_load_local(&mut self, idx: u16) -> Result<Option<usize>, NyarError> {
        let f = self.frames.last().unwrap();
        if (idx as usize) < f.locals.len() {
            if let Some(up) = f.upvalues.get(idx as usize).and_then(|x| x.as_ref()) {
                self.push(up.get())?;
            } else {
                let v = f.locals[idx as usize];
                self.push(v)?;
            }
            Ok(None)
        } else {
            Err(self.error(nyar_types::VmErrorKind::StackUnderflow))
        }
    }

    #[inline(always)]
    pub fn execute_store_local(&mut self, idx: u16) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let gc = &self.gc;
        let f = self.frames.last_mut().unwrap();
        if (idx as usize) >= f.locals.len() {
            f.locals.resize((idx as usize) + 1, Value::null());
            f.upvalues.resize((idx as usize) + 1, None);
        }
        if let Some(up) = f.upvalues[idx as usize].as_ref() {
            up.set(v);
        } else {
            f.locals[idx as usize] = v;
        }
        v.write_barrier(gc);
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_load_global(&mut self, name_idx: u16, module_idx: usize) -> Result<Option<usize>, NyarError> {
        let module = &self.modules[module_idx];
        let name = match module.constants.get(name_idx as usize) {
            Some(Constant::QualifiedName(qn)) => qn.clone(),
            Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
            _ => return Err(self.error(nyar_types::VmErrorKind::InvalidOpcode(0x07))),
        };
        let val = if let Some(v) = self.builtins.get(&name) {
            Some(*v)
        } else if let Some(res) = self.symbol_table.get(&name) {
            Some(Value::null())
        } else {
            None
        };

        if let Some(v) = val {
            self.push(v)?;
            Ok(None)
        } else {
            Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(name)))
        }
    }

    #[inline(always)]
    pub fn execute_store_global(&mut self, name_idx: u16, module_idx: usize) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let gc = &self.gc;
        let module = &self.modules[module_idx];
        let name = match module.constants.get(name_idx as usize) {
            Some(Constant::QualifiedName(qn)) => qn.clone(),
            Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
            _ => return Err(self.error(nyar_types::VmErrorKind::InvalidOpcode(0x08))),
        };
        self.builtins.insert(name, v);
        v.write_barrier(gc);
        Ok(None)
    }
}
