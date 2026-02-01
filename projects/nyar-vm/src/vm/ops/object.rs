use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_new_object(&mut self, class_idx: u16) -> Result<Option<usize>, VmError> {
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
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_get_field(&mut self, idx: u16) -> Result<Option<usize>, VmError> {
        let obj_val = self.pop()?;
        let obj = unsafe { obj_val.as_object() };
        if (idx as usize) < obj.fields.len() {
            self.push(obj.fields[idx as usize])?;
            Ok(None)
        } else {
            Err(VmError::IndexOutOfBounds)
        }
    }

    #[inline(always)]
    pub fn execute_set_field(&mut self, idx: u16) -> Result<Option<usize>, VmError> {
        let val = self.pop()?;
        let obj_val = self.pop()?;
        let gc = &self.gc;
        let obj = unsafe { obj_val.as_object_mut() };
        if (idx as usize) < obj.fields.len() {
            obj.fields[idx as usize] = val;
            val.write_barrier(gc);
            self.push(obj_val)?;
            Ok(None)
        } else {
            Err(VmError::IndexOutOfBounds)
        }
    }

    #[inline(always)]
    pub fn execute_new_array(&mut self, len: u32) -> Result<Option<usize>, VmError> {
        let mut items = Vec::with_capacity(len as usize);
        for _ in 0..len {
            items.push(self.pop()?);
        }
        items.reverse();
        let arr = Value::array(items, &self.gc);
        self.push(arr)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_get_element(&mut self) -> Result<Option<usize>, VmError> {
        let key_val = self.pop()?;
        let arr_val = self.pop()?;

        if arr_val.is_dyn_object() {
            let key = key_val.try_as_str().ok_or(VmError::InvalidOpcode)?;
            let obj = unsafe { arr_val.as_dyn_object() };
            let val = obj.entries.get(key).cloned().unwrap_or(Value::null());
            self.push(val)?;
            Ok(None)
        } else {
            let idx = key_val.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
            if arr_val.is_array() {
                let arr = unsafe { arr_val.as_array() };
                if idx < arr.items.len() {
                    self.push(arr.items[idx])?;
                    Ok(None)
                } else {
                    Err(VmError::IndexOutOfBounds)
                }
            } else if arr_val.is_list() {
                let list = unsafe { arr_val.as_list() };
                if idx < list.items.len() {
                    self.push(list.items[idx])?;
                    Ok(None)
                } else {
                    Err(VmError::IndexOutOfBounds)
                }
            } else if arr_val.is_tuple() {
                let tuple = unsafe { arr_val.as_tuple() };
                if idx < tuple.items.len() {
                    self.push(tuple.items[idx])?;
                    Ok(None)
                } else {
                    Err(VmError::IndexOutOfBounds)
                }
            } else {
                Err(VmError::InvalidOpcode)
            }
        }
    }

    #[inline(always)]
    pub fn execute_set_element(&mut self) -> Result<Option<usize>, VmError> {
        let val = self.pop()?;
        let key_val = self.pop()?;
        let arr_val = self.pop()?;

        let gc = &self.gc;
        if arr_val.is_dyn_object() {
            let key = key_val.try_as_str().ok_or(VmError::InvalidOpcode)?;
            let obj = unsafe { arr_val.as_dyn_object_mut() };
            obj.entries.insert(key.to_string(), val);
            val.write_barrier(gc);
            self.push(arr_val)?;
            Ok(None)
        } else {
            let idx = key_val.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
            if arr_val.is_array() {
                let arr = unsafe { arr_val.as_array_mut() };
                if idx < arr.items.len() {
                    arr.items[idx] = val;
                    val.write_barrier(gc);
                    self.push(arr_val)?;
                    Ok(None)
                } else {
                    Err(VmError::IndexOutOfBounds)
                }
            } else if arr_val.is_list() {
                let list = unsafe { arr_val.as_list_mut() };
                if idx < list.items.len() {
                    list.items[idx] = val;
                    val.write_barrier(gc);
                    self.push(arr_val)?;
                    Ok(None)
                } else {
                    Err(VmError::IndexOutOfBounds)
                }
            } else if arr_val.is_tuple() {
                let tuple = unsafe { arr_val.as_tuple_mut() };
                if idx < tuple.items.len() {
                    if tuple.items[idx].tag() != val.tag() {
                        return Err(VmError::RuntimeError("Tuple element type mismatch".to_string()));
                    }
                    tuple.items[idx] = val;
                    val.write_barrier(gc);
                    self.push(arr_val)?;
                    Ok(None)
                } else {
                    Err(VmError::IndexOutOfBounds)
                }
            } else {
                Err(VmError::InvalidOpcode)
            }
        }
    }

    #[inline(always)]
    pub fn execute_new_dyn_object(&mut self) -> Result<Option<usize>, VmError> {
        let obj = Value::dyn_object(&self.gc);
        self.push(obj)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_new_list(&mut self, len: u32) -> Result<Option<usize>, VmError> {
        let mut items = Vec::with_capacity(len as usize);
        for _ in 0..len {
            items.push(self.pop()?);
        }
        items.reverse();
        let list = Value::list(items, &self.gc);
        self.push(list)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_make_tuple(&mut self, len: u32) -> Result<Option<usize>, VmError> {
        let mut items = Vec::with_capacity(len as usize);
        for _ in 0..len {
            items.push(self.pop()?);
        }
        items.reverse();
        let tuple = Value::tuple(items, &self.gc);
        self.push(tuple)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_has_key(&mut self) -> Result<Option<usize>, VmError> {
        let key = self.pop()?;
        let obj_val = self.pop()?;
        if obj_val.is_dyn_object() {
            let key_str = key.try_as_str().ok_or(VmError::InvalidOpcode)?;
            let obj = unsafe { obj_val.as_dyn_object() };
            self.push(Value::bool(obj.entries.contains_key(key_str)))?;
            Ok(None)
        } else if obj_val.is_object() {
            let key_str = key.try_as_str().ok_or(VmError::InvalidOpcode)?;
            let obj = unsafe { obj_val.as_object() };
            let frame = self.frames.last().ok_or(VmError::StackUnderflow)?;
            let class_info = self.modules[frame.module_idx]
                .classes
                .get(obj.class_idx as usize)
                .ok_or(VmError::IndexOutOfBounds)?;
            let has_field = class_info.fields.iter().any(|f| f == key_str);
            self.push(Value::bool(has_field))?;
            Ok(None)
        } else if obj_val.is_array() {
            let idx = key.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
            let arr = unsafe { obj_val.as_array() };
            self.push(Value::bool(idx < arr.items.len()))?;
            Ok(None)
        } else if obj_val.is_list() {
            let idx = key.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
            let list = unsafe { obj_val.as_list() };
            self.push(Value::bool(idx < list.items.len()))?;
            Ok(None)
        } else if obj_val.is_tuple() {
            let idx = key.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
            let tuple = unsafe { obj_val.as_tuple() };
            self.push(Value::bool(idx < tuple.items.len()))?;
            Ok(None)
        } else {
            Err(VmError::InvalidOpcode)
        }
    }

    #[inline(always)]
    pub fn execute_remove_key(&mut self) -> Result<Option<usize>, VmError> {
        let key = self.pop()?;
        let obj_val = self.pop()?;
        if obj_val.is_dyn_object() {
            let key_str = key.try_as_str().ok_or(VmError::InvalidOpcode)?;
            let obj = unsafe { obj_val.as_dyn_object_mut() };
            let removed = obj.entries.remove(key_str);
            self.push(obj_val)?;
            self.push(removed.unwrap_or(Value::null()))?;
            Ok(None)
        } else if obj_val.is_list() {
            let idx = key.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
            let list = unsafe { obj_val.as_list_mut() };
            if idx < list.items.len() {
                let removed = list.items.remove(idx);
                self.push(obj_val)?;
                self.push(removed)?;
                Ok(None)
            } else {
                Err(VmError::IndexOutOfBounds)
            }
        } else {
            Err(VmError::InvalidOpcode)
        }
    }

    #[inline(always)]
    pub fn execute_push_element_right(&mut self) -> Result<Option<usize>, VmError> {
        let val = self.pop()?;
        let list_val = self.pop()?;
        let gc = &self.gc;
        if list_val.is_list() {
            let list = unsafe { list_val.as_list_mut() };
            list.items.push(val);
            val.write_barrier(gc);
            self.push(list_val)?;
            Ok(None)
        } else if list_val.is_array() {
            let arr = unsafe { list_val.as_array_mut() };
            arr.items.push(val);
            val.write_barrier(gc);
            self.push(list_val)?;
            Ok(None)
        } else {
            Err(VmError::InvalidOpcode)
        }
    }

    #[inline(always)]
    pub fn execute_pop_element_right(&mut self) -> Result<Option<usize>, VmError> {
        let list_val = self.pop()?;
        if list_val.is_list() {
            let list = unsafe { list_val.as_list_mut() };
            let val = list.items.pop().ok_or(VmError::IndexOutOfBounds)?;
            self.push(list_val)?;
            self.push(val)?;
            Ok(None)
        } else if list_val.is_array() {
            let arr = unsafe { list_val.as_array_mut() };
            let val = arr.items.pop().ok_or(VmError::IndexOutOfBounds)?;
            self.push(list_val)?;
            self.push(val)?;
            Ok(None)
        } else {
            Err(VmError::InvalidOpcode)
        }
    }

    #[inline(always)]
    pub fn execute_push_element_left(&mut self) -> Result<Option<usize>, VmError> {
        let val = self.pop()?;
        let list_val = self.pop()?;
        let gc = &self.gc;
        if list_val.is_list() {
            let list = unsafe { list_val.as_list_mut() };
            list.items.insert(0, val);
            val.write_barrier(gc);
            self.push(list_val)?;
            Ok(None)
        } else if list_val.is_array() {
            let arr = unsafe { list_val.as_array_mut() };
            arr.items.insert(0, val);
            val.write_barrier(gc);
            self.push(list_val)?;
            Ok(None)
        } else {
            Err(VmError::InvalidOpcode)
        }
    }

    #[inline(always)]
    pub fn execute_pop_element_left(&mut self) -> Result<Option<usize>, VmError> {
        let list_val = self.pop()?;
        if list_val.is_list() {
            let list = unsafe { list_val.as_list_mut() };
            if list.items.is_empty() {
                return Err(VmError::IndexOutOfBounds);
            }
            let val = list.items.remove(0);
            self.push(list_val)?;
            self.push(val)?;
            Ok(None)
        } else if list_val.is_array() {
            let arr = unsafe { list_val.as_array_mut() };
            if arr.items.is_empty() {
                return Err(VmError::IndexOutOfBounds);
            }
            let val = arr.items.remove(0);
            self.push(list_val)?;
            self.push(val)?;
            Ok(None)
        } else {
            Err(VmError::InvalidOpcode)
        }
    }

    #[inline(always)]
    pub fn execute_size_of(&mut self) -> Result<Option<usize>, VmError> {
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
        } else if val.is_object() {
            unsafe { val.as_object().fields.len() }
        } else if val.is_bigint() {
            unsafe { val.as_bigint().0.to_bytes_le().1.len() }
        } else {
            return Err(VmError::InvalidOpcode);
        };
        self.push(Value::int(size as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_type_of(&mut self) -> Result<Option<usize>, VmError> {
        let val = self.pop()?;
        let tag = val.tag();
        self.push(Value::int(tag as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_instance_of(&mut self, class_idx: u16) -> Result<Option<usize>, VmError> {
        let val = self.pop()?;
        let is_instance = if val.is_object() {
            let obj = unsafe { val.as_object() };
            obj.class_idx == class_idx
        } else {
            false
        };
        self.push(Value::bool(is_instance))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_match_variant(&mut self, class_idx: u16) -> Result<Option<usize>, VmError> {
        let val = self.peek_at(0)?;
        let matches = if val.is_object() {
            let obj = unsafe { val.as_object() };
            obj.class_idx == class_idx
        } else {
            false
        };
        self.push(Value::bool(matches))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_check_cast(&mut self, class_idx: u16) -> Result<Option<usize>, VmError> {
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
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_cast(&mut self, class_idx: u16) -> Result<Option<usize>, VmError> {
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
        Ok(None)
    }
}
