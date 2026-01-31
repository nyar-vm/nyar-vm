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
                    self.push(obj)?;
                }
            Instruction::GetField(idx) => {
                let obj_val = self.pop()?;
                let obj = unsafe { obj_val.as_object() };
                if (idx as usize) < obj.fields.len() {
                    self.push(obj.fields[idx as usize])?;
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
                self.push(arr)?;
            }
            Instruction::GetElement => {
                let idx_val = self.pop()?;
                let arr_val = self.pop()?;
                let idx = idx_val.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;

                if arr_val.is_array() {
                    let arr = unsafe { arr_val.as_array() };
                    if idx < arr.items.len() {
                        self.push(arr.items[idx])?;
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                } else if arr_val.is_list() {
                    let list = unsafe { arr_val.as_list() };
                    if idx < list.items.len() {
                        self.push(list.items[idx])?;
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                } else if arr_val.is_tuple() {
                    let tuple = unsafe { arr_val.as_tuple() };
                    if idx < tuple.items.len() {
                        self.push(tuple.items[idx])?;
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
                self.push(obj)?;
            }
            Instruction::NewList(len) => {
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(self.pop()?);
                }
                items.reverse();
                let list = Value::list(items, &self.gc);
                self.push(list)?;
            }
            Instruction::MakeTuple(len) => {
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(self.pop()?);
                }
                items.reverse();
                let tuple = Value::tuple(items, &self.gc);
                self.push(tuple)?;
            }
            Instruction::HasKey => {
                let key = self.pop()?;
                let obj_val = self.pop()?;
                if obj_val.is_dyn_object() {
                    let key_str = key.try_as_str().ok_or(VmError::InvalidOpcode)?;
                    let obj = unsafe { obj_val.as_dyn_object() };
                    self.push(Value::bool(obj.entries.contains_key(key_str)))?;
                } else if obj_val.is_array() {
                    let idx = key.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
                    let arr = unsafe { obj_val.as_array() };
                    self.push(Value::bool(idx < arr.items.len()))?;
                } else if obj_val.is_list() {
                    let idx = key.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
                    let list = unsafe { obj_val.as_list() };
                    self.push(Value::bool(idx < list.items.len()))?;
                } else if obj_val.is_tuple() {
                    let idx = key.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
                    let tuple = unsafe { obj_val.as_tuple() };
                    self.push(Value::bool(idx < tuple.items.len()))?;
                } else {
                    return Err(VmError::InvalidOpcode);
                }
            }
            Instruction::RemoveKey => {
                let key = self.pop()?;
                let obj_val = self.pop()?;
                if obj_val.is_dyn_object() {
                    let key_str = key.try_as_str().ok_or(VmError::InvalidOpcode)?;
                    let obj = unsafe { obj_val.as_dyn_object_mut() };
                    let removed = obj.entries.remove(key_str);
                    self.push(removed.unwrap_or(Value::null()))?;
                } else if obj_val.is_list() {
                    let idx = key.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
                    let list = unsafe { obj_val.as_list_mut() };
                    if idx < list.items.len() {
                        let removed = list.items.remove(idx);
                        self.push(removed)?;
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                } else {
                    return Err(VmError::InvalidOpcode);
                }
            }
            Instruction::PushElementRight => {
                let val = self.pop()?;
                let list_val = self.pop()?;
                let gc = &self.gc;
                if list_val.is_list() {
                    let list = unsafe { list_val.as_list_mut() };
                    list.items.push(val);
                    val.write_barrier(gc);
                } else {
                    return Err(VmError::InvalidOpcode);
                }
            }
            Instruction::PopElementRight => {
                let list_val = self.pop()?;
                if list_val.is_list() {
                    let list = unsafe { list_val.as_list_mut() };
                    let val = list.items.pop().ok_or(VmError::IndexOutOfBounds)?;
                    self.push(val)?;
                } else {
                    return Err(VmError::InvalidOpcode);
                }
            }
            Instruction::PushElementLeft => {
                let val = self.pop()?;
                let list_val = self.pop()?;
                let gc = &self.gc;
                if list_val.is_list() {
                    let list = unsafe { list_val.as_list_mut() };
                    list.items.insert(0, val);
                    val.write_barrier(gc);
                } else {
                    return Err(VmError::InvalidOpcode);
                }
            }
            Instruction::PopElementLeft => {
                let list_val = self.pop()?;
                if list_val.is_list() {
                    let list = unsafe { list_val.as_list_mut() };
                    if list.items.is_empty() {
                        return Err(VmError::IndexOutOfBounds);
                    }
                    let val = list.items.remove(0);
                    self.push(val)?;
                } else {
                    return Err(VmError::InvalidOpcode);
                }
            }
            Instruction::SizeOf => {
                let val = self.pop()?;
                let size = if val.is_array() {
                    unsafe { val.as_array().items.len() }
                } else if val.is_list() {
                    unsafe { val.as_list().items.len() }
                } else if val.is_tuple() {
                    unsafe { val.as_tuple().items.len() }
                } else if val.is_string() {
                    unsafe { val.as_string().len() }
                } else if val.is_dyn_object() {
                    unsafe { val.as_dyn_object().entries.len() }
                } else {
                    return Err(VmError::InvalidOpcode);
                };
                self.push(Value::int(size as i64))?;
            }
            Instruction::TypeOf => {
                let val = self.pop()?;
                let tag = val.tag();
                self.push(Value::int(tag as i64))?;
            }
            Instruction::InstanceOf(class_idx) => {
                let val = self.pop()?;
                let is_instance = if val.is_object() {
                    let obj = unsafe { val.as_object() };
                    obj.class_idx == class_idx
                } else {
                    false
                };
                self.push(Value::bool(is_instance))?;
            }
            Instruction::MatchVariant(class_idx) => {
                let val = self.peek_at(0)?;
                let matches = if val.is_object() {
                    let obj = unsafe { val.as_object() };
                    obj.class_idx == class_idx
                } else {
                    false
                };
                self.push(Value::bool(matches))?;
            }
            Instruction::CheckCast(class_idx) => {
                let val = self.peek_at(0)?;
                if val.is_object() {
                    let obj = unsafe { val.as_object() };
                    if obj.class_idx != class_idx {
                        return Err(VmError::RuntimeError(format!(
                            "Invalid cast: expected class {}, got {}",
                            class_idx, obj.class_idx
                        )));
                    }
                } else {
                    return Err(VmError::RuntimeError(format!(
                        "Invalid cast: expected class {}, got tag {:?}",
                        class_idx,
                        val.tag()
                    )));
                }
            }
            Instruction::Cast(class_idx) => {
                // For now, Cast is same as CheckCast but could be extended for more complex conversions
                let val = self.peek_at(0)?;
                if val.is_object() {
                    let obj = unsafe { val.as_object() };
                    if obj.class_idx != class_idx {
                        return Err(VmError::RuntimeError(format!(
                            "Invalid cast: expected class {}, got {}",
                            class_idx, obj.class_idx
                        )));
                    }
                } else {
                    return Err(VmError::RuntimeError(format!(
                        "Invalid cast: expected class {}, got tag {:?}",
                        class_idx,
                        val.tag()
                    )));
                }
            }
            _ => return Err(VmError::InvalidOpcode),
        }
        Ok(())
    }
}
