use crate::vm::core::NyarVM;
use crate::vm::value::{Upvalue, Value};
use crate::bytecode::format::Constant;

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_get_field(_vm_ptr: *mut NyarVM, obj_val: Value, idx: u32) -> Value {
    let obj = obj_val.as_object();
    if (idx as usize) < obj.fields.len() {
        obj.fields[idx as usize]
    } else {
        Value::null()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_set_field(vm_ptr: *mut NyarVM, obj_val: Value, idx: u32, val: Value) {
    let vm = &mut *vm_ptr;
    let obj = obj_val.as_object_mut();
    if (idx as usize) < obj.fields.len() {
        obj.fields[idx as usize] = val;
        val.write_barrier(&vm.gc);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_get_element(_vm_ptr: *mut NyarVM, arr_val: Value, idx_val: Value) -> Value {
    let idx = idx_val.try_as_int().unwrap_or(0) as usize;
    if arr_val.is_array() {
        let arr = arr_val.as_array();
        if idx < arr.items.len() {
            return arr.items[idx];
        }
    } else if arr_val.is_list() {
        let list = arr_val.as_list();
        if idx < list.items.len() {
            return list.items[idx];
        }
    } else if arr_val.is_tuple() {
        let tuple = arr_val.as_tuple();
        if idx < tuple.items.len() {
            return tuple.items[idx];
        }
    }
    Value::null()
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_set_element(vm_ptr: *mut NyarVM, arr_val: Value, idx_val: Value, val: Value) {
    let vm = &mut *vm_ptr;
    let idx = idx_val.try_as_int().unwrap_or(0) as usize;
    if arr_val.is_array() {
        let arr = arr_val.as_array_mut();
        if idx < arr.items.len() {
            arr.items[idx] = val;
            val.write_barrier(&vm.gc);
        }
    } else if arr_val.is_list() {
        let list = arr_val.as_list_mut();
        if idx < list.items.len() {
            list.items[idx] = val;
            val.write_barrier(&vm.gc);
        }
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_str_concat(vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> Value {
    let vm = &mut *vm_ptr;
    let s1 = lhs.try_as_str().unwrap_or("");
    let s2 = rhs.try_as_str().unwrap_or("");
    let res = format!("{}{}", s1, s2);
    Value::string(res, &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_size_of(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
    if val.is_array() {
        val.as_array().items.len() as i64
    } else if val.is_list() {
        val.as_list().items.len() as i64
    } else if val.is_tuple() {
        val.as_tuple().items.len() as i64
    } else if val.is_string() {
        val.try_as_str().unwrap_or("").len() as i64
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_cast_to(_vm_ptr: *mut NyarVM, val: Value, _type_idx: u32) -> Value {
    val
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_new_object(vm_ptr: *mut NyarVM, class_idx: u32, fields_count: u32) -> Value {
    let vm = &mut *vm_ptr;
    let mut fields = Vec::with_capacity(fields_count as usize);
    for _ in 0..fields_count {
        fields.push(vm.pop().unwrap());
    }
    fields.reverse();
    let obj = Value::object(class_idx as u16, fields, &vm.gc);
    vm.push(obj).unwrap();
    obj
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_new_array(vm_ptr: *mut NyarVM, len: u32) -> Value {
    let vm = &mut *vm_ptr;
    let mut items = Vec::with_capacity(len as usize);
    for _ in 0..len {
        items.push(vm.pop().unwrap());
    }
    items.reverse();
    let arr = Value::array(items, &vm.gc);
    vm.push(arr).unwrap();
    arr
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_make_tuple(vm_ptr: *mut NyarVM, len: u32) -> Value {
    let vm = &mut *vm_ptr;
    let mut items = Vec::with_capacity(len as usize);
    for _ in 0..len {
        items.push(vm.pop().unwrap());
    }
    items.reverse();
    let tuple = Value::tuple(items, &vm.gc);
    vm.push(tuple).unwrap();
    tuple
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_make_closure(vm_ptr: *mut NyarVM, func_idx: u32, capture_count: u32) -> Value {
    let vm = &mut *vm_ptr;
    let mut captures = Vec::with_capacity(capture_count as usize);
    for _ in 0..capture_count {
        captures.push(Upvalue(vm.pop().unwrap()));
    }
    captures.reverse();
    let module_idx = vm.frames.last().unwrap().module_idx;
    let closure = Value::closure(module_idx, func_idx as u16, captures, &vm.gc);
    vm.push(closure).unwrap();
    closure
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call_closure(vm_ptr: *mut NyarVM, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    vm.execute_call_closure(argc as u16).unwrap().unwrap_or(Value::null())
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_tail_call(_vm_ptr: *mut NyarVM, _target: Value) -> i32 {
    3 // StatusTailCall
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_capture_ref(vm_ptr: *mut NyarVM, idx: u32, is_local: bool) -> Value {
    let vm = &mut *vm_ptr;
    if is_local {
        let frame = vm.frames.last().unwrap();
        frame.locals[idx as usize]
    } else {
        let frame = vm.frames.last().unwrap();
        let closure = frame.closure.as_closure();
        closure.upvalues[idx as usize].0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_load_global(vm_ptr: *mut NyarVM, name_idx: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let module = &vm.modules[module_idx];
    let name = match module.constants.get(name_idx as usize) {
        Some(Constant::String(s)) => s,
        _ => return Value::null(),
    };
    if let Some(v) = vm.builtins.get(name) {
        let val = *v;
        vm.push(val).unwrap();
        val
    } else if let Some(&(_m_idx, _c_idx)) = vm.symbol_table.get(name) {
        let val = Value::null();
        vm.push(val).unwrap();
        val
    } else {
        Value::null()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call_symbol(vm_ptr: *mut NyarVM, name_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    vm.execute_call_symbol(name_idx as u16, argc as u16, module_idx).unwrap().unwrap_or(Value::null())
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_invoke_method(vm_ptr: *mut NyarVM, name_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    vm.execute_invoke_method(name_idx as u16, argc as u16, module_idx).unwrap().unwrap_or(Value::null())
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_store_global(vm_ptr: *mut NyarVM, name_idx: u32, val: Value) {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let module = &vm.modules[module_idx];
    let name = match module.constants.get(name_idx as usize) {
        Some(Constant::String(s)) => s.clone(),
        _ => return,
    };
    vm.builtins.insert(name, val);
    val.write_barrier(&vm.gc);
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call(vm_ptr: *mut NyarVM, chunk_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    vm.execute_call(chunk_idx as u16, argc as u16, module_idx).unwrap();
    Value::null()
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call_virtual(vm_ptr: *mut NyarVM, name_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    // CallVirtual is often similar to InvokeMethod but on a specific class or interface
    // For now, let's use execute_invoke_method as a fallback if possible, or just return null
    vm.execute_invoke_method(name_idx as u16, argc as u16, module_idx).unwrap().unwrap_or(Value::null())
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call_dynamic(vm_ptr: *mut NyarVM, name_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    // CallDynamic is for dynamic dispatch
    vm.execute_invoke_method(name_idx as u16, argc as u16, module_idx).unwrap().unwrap_or(Value::null())
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_close_upvalues(vm_ptr: *mut NyarVM) {
    let _vm = &mut *vm_ptr;
    // Placeholder for CloseUpvalues
}
