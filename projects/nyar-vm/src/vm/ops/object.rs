use crate::bytecode::instruction::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_object_op(&mut self, ins: Instruction) -> Result<(), VmError> {
        match ins {
            Instruction::NewObject(class_idx) => {
                    let frame = self.frames.last().ok_or(VmError::StackUnderflow)?;
                    let class_info = self.modules[frame.module_idx]
                        .classes
                        .get(class_idx as usize)
                        .ok_or(VmError::IndexOutOfBounds)?;
                    let fields_count = class_info.fields.len();
                    let mut fields = Vec::with_capacity(fields_count);
                    for _ in 0..fields_count {
                        fields.push(self.pop()?);
                    }
                    fields.reverse();
                    let obj = Value::object(class_idx, fields, &self.gc);
                    self.push(obj);
                }
            Instruction::GetField(idx) => {
                let obj_val = self.pop()?;
                let obj = obj_val.try_as_object().ok_or(VmError::InvalidOpcode)?;
                if (idx as usize) < obj.fields.len() {
                    self.push(obj.fields[idx as usize]);
                } else {
                    return Err(VmError::IndexOutOfBounds);
                }
            }
            Instruction::SetField(idx) => {
                let val = self.pop()?;
                let obj_val = self.pop()?;
                let gc = &self.gc;
                let obj = unsafe { obj_val.as_object_mut() };
                if (idx as usize) < obj.fields.len() {
                    obj.fields[idx as usize] = val;
                    // Trigger write barrier for GC safety
                    val.write_barrier(gc);
                } else {
                    return Err(VmError::IndexOutOfBounds);
                }
            }
            Instruction::NewArray(len) => {
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(self.pop()?);
                }
                items.reverse();
                let arr = Value::array(items, &self.gc);
                self.push(arr);
            }
            Instruction::GetElement => {
                let idx_val = self.pop()?;
                let arr_val = self.pop()?;
                let idx = idx_val.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
                
                if arr_val.is_array() {
                    let arr = unsafe { arr_val.as_array() };
                    if idx < arr.items.len() {
                        self.push(arr.items[idx]);
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                } else if arr_val.is_list() {
                    let list = unsafe { arr_val.as_list() };
                    if idx < list.items.len() {
                        self.push(list.items[idx]);
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                } else {
                    return Err(VmError::InvalidOpcode);
                }
            }
            Instruction::SetElement => {
                let val = self.pop()?;
                let idx_val = self.pop()?;
                let arr_val = self.pop()?;
                let gc = &self.gc;
                let idx = idx_val.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;

                if arr_val.is_array() {
                    let arr = unsafe { arr_val.as_array_mut() };
                    if idx < arr.items.len() {
                        arr.items[idx] = val;
                        val.write_barrier(gc);
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                } else if arr_val.is_list() {
                    let list = unsafe { arr_val.as_list_mut() };
                    if idx < list.items.len() {
                        list.items[idx] = val;
                        val.write_barrier(gc);
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                } else {
                    return Err(VmError::InvalidOpcode);
                }
            }
            Instruction::NewDynObject => {
                let obj = Value::dyn_object(&self.gc);
                self.push(obj);
            }
            Instruction::NewList(len) => {
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(self.pop()?);
                }
                items.reverse();
                let list = Value::list(items, &self.gc);
                self.push(list);
            }
            Instruction::MakeTuple(len) => {
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(self.pop()?);
                }
                items.reverse();
                let tuple = Value::tuple(items, &self.gc);
                self.push(tuple);
            }
            _ => return Err(VmError::InvalidOpcode),
        }
        Ok(())
    }
}
