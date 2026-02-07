use crate::vm::core::NyarVM;
use crate::vm::value::{BigInt, Upvalue, Value};
use crate::bytecode::format::Constant;
use nyar_types::QualifiedName;

#[no_mangle]
pub unsafe extern "win64" fn nyar_upvalue_get(upvalue_ptr: *const Option<Upvalue>) -> Value {
    if let Some(up) = &*upvalue_ptr {
        up.get()
    } else {
        Value::null()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_upvalue_set(upvalue_ptr: *mut Option<Upvalue>, val: Value) {
    if let Some(up) = &*upvalue_ptr {
        up.set(val);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_load_local(vm_ptr: *mut NyarVM, idx: u32) -> Value {
    let vm = &mut *vm_ptr;
    let f = vm.frames.last().unwrap();
    if (idx as usize) < f.locals.len() {
        if let Some(up) = f.upvalues.get(idx as usize).and_then(|x| x.as_ref()) {
            up.get()
        } else {
            f.locals[idx as usize]
        }
    } else {
        Value::null()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_store_local(vm_ptr: *mut NyarVM, idx: u32, val: Value) {
    let vm = &mut *vm_ptr;
    let gc = &vm.gc;
    let f = vm.frames.last_mut().unwrap();
    if (idx as usize) >= f.locals.len() {
        f.locals.resize((idx as usize) + 1, Value::null());
        f.upvalues.resize((idx as usize) + 1, None);
    }
    if let Some(up) = f.upvalues[idx as usize].as_ref() {
        up.set(val);
    } else {
        f.locals[idx as usize] = val;
    }
    val.write_barrier(gc);
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_make_closure(
    vm_ptr: *mut NyarVM,
    module_idx: u32,
    func_idx: u32,
    upvalues_ptr: *const crate::bytecode::instruction::UpvalueRef,
    upvalues_count: u32,
) -> Value {
    let vm = &mut *vm_ptr;
    let upvalues = std::slice::from_raw_parts(upvalues_ptr, upvalues_count as usize).to_vec();
    
    let mut captured = Vec::with_capacity(upvalues.len());
    for up in upvalues {
        let upvalue = if up.is_local {
            let f = vm.frames.last_mut().unwrap();
            let index = up.index as usize;
            if let Some(existing) = f.upvalues.get(index).and_then(|x| x.as_ref()) {
                existing.clone()
            } else {
                let new_up = Upvalue::new(f.locals[index]);
                if index >= f.upvalues.len() {
                    f.upvalues.resize(index + 1, None);
                }
                f.upvalues[index] = Some(new_up.clone());
                new_up
            }
        } else {
            let f = vm.frames.last().unwrap();
            let closure = f.closure.as_closure();
            closure.upvalues[up.index as usize].clone()
        };
        captured.push(upvalue);
    }
    Value::closure(module_idx as usize, func_idx as u16, captured, &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_get_field(_vm_ptr: *mut NyarVM, obj_val: Value, idx: u32) -> Value {
    let obj = obj_val.as_object();
    obj.fields.get(idx as usize).cloned().unwrap_or(Value::null())
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
pub unsafe extern "win64" fn nyar_vm_get_element(_vm_ptr: *mut NyarVM, obj: Value, idx: Value) -> Value {
    if obj.is_list() {
        let l = obj.as_list();
        let i = idx.as_int() as usize;
        l.items.get(i).cloned().unwrap_or(Value::null())
    } else if obj.is_array() {
        let a = obj.as_array();
        let i = idx.as_int() as usize;
        a.items.get(i).cloned().unwrap_or(Value::null())
    } else if obj.is_tuple() {
        let t = obj.as_tuple();
        let i = idx.as_int() as usize;
        t.items.get(i).cloned().unwrap_or(Value::null())
    } else if obj.is_dyn_object() {
        let o = obj.as_dyn_object();
        let key = idx.try_as_str().unwrap_or("");
        o.entries.get(key).cloned().unwrap_or(Value::null())
    } else {
        Value::null()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_set_element(vm_ptr: *mut NyarVM, obj: Value, idx: Value, val: Value) {
    let vm = &mut *vm_ptr;
    if obj.is_list() {
        let l = obj.as_list_mut();
        let i = idx.as_int() as usize;
        if i < l.items.len() {
            l.items[i] = val;
            val.write_barrier(&vm.gc);
        }
    } else if obj.is_array() {
        let a = obj.as_array_mut();
        let i = idx.as_int() as usize;
        if i < a.items.len() {
            a.items[i] = val;
            val.write_barrier(&vm.gc);
        }
    } else if obj.is_dyn_object() {
        let o = obj.as_dyn_object_mut();
        let key = idx.try_as_str().unwrap_or("");
        o.entries.insert(key.to_string(), val);
        val.write_barrier(&vm.gc);
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_new_object(vm_ptr: *mut NyarVM, class_idx: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let module = &vm.modules[module_idx];
    if (class_idx as usize) >= module.classes.len() {
        return Value::null();
    }
    let class_info = &module.classes[class_idx as usize];
    let fields_count = class_info.fields.len();
    let mut fields = Vec::with_capacity(fields_count);
    for _ in 0..fields_count {
        fields.push(vm.pop().unwrap_or(Value::null()));
    }
    fields.reverse();
    let obj = Value::object(module_idx, class_idx as u16, fields, &vm.gc);
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
pub unsafe extern "win64" fn nyar_vm_new_dyn_object(vm_ptr: *mut NyarVM) -> Value {
    let vm = &mut *vm_ptr;
    let obj = Value::dyn_object(&vm.gc);
    vm.push(obj).unwrap();
    obj
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
pub unsafe extern "win64" fn nyar_vm_string_concat(vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> Value {
    let vm = &mut *vm_ptr;
    let l = lhs.try_as_str().unwrap_or("");
    let r = rhs.try_as_str().unwrap_or("");
    let mut s = String::with_capacity(l.len() + r.len());
    s.push_str(l);
    s.push_str(r);
    Value::string(s, &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_type_of(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
    val.tag() as i64
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_size_of(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
    if val.is_list() {
        val.as_list().items.len() as i64
    } else if val.is_array() {
        val.as_array().items.len() as i64
    } else if val.is_tuple() {
        val.as_tuple().items.len() as i64
    } else if val.is_string() {
        val.as_string().len() as i64
    } else if val.is_dyn_object() {
        val.as_dyn_object().entries.len() as i64
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_cast_to(_vm_ptr: *mut NyarVM, val: Value, _type_idx: u32) -> Value {
    val
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_perform_effect(vm_ptr: *mut NyarVM, idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    match vm.execute_perform(idx as u16, argc as u8, module_idx) {
        Ok(_) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[inline(always)]
unsafe fn drive_until(vm: &mut NyarVM, target_depth: usize) -> Value {
    while vm.frames.len() > target_depth {
        match vm.execute_step() {
            Ok(Some(())) => continue,
            Ok(None) => break,
            Err(e) if matches!(*e.kind, nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::YieldAsync)) => {
                std::thread::yield_now();
                continue;
            }
            Err(e) => {
                vm.print_traceback(&e);
                return Value::null();
            }
        }
    }
    vm.pop().unwrap_or(Value::null())
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_ffi_call(vm_ptr: *mut NyarVM, idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    match vm.execute_ffi_call(idx as u16, argc as u8, module_idx) {
        Ok(Some(_)) => drive_until(vm, vm.frames.len() - 1),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_await(vm_ptr: *mut NyarVM) -> Value {
    let vm = &mut *vm_ptr;
    match vm.execute_await() {
        Ok(Some(_)) => drive_until(vm, vm.frames.len() - 1),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_block_on(vm_ptr: *mut NyarVM) -> Value {
    let vm = &mut *vm_ptr;
    match vm.execute_block_on() {
        Ok(Some(_)) => drive_until(vm, vm.frames.len() - 1),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call_closure(vm_ptr: *mut NyarVM, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let depth = vm.frames.len();
    match vm.execute_call_closure(argc as u16) {
        Ok(Some(_)) => drive_until(vm, depth),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_load_upvalue(_vm_ptr: *mut NyarVM, closure_val: Value, idx: u32) -> Value {
    let closure = closure_val.as_closure();
    closure.upvalues.get(idx as usize).map(|u| u.get()).unwrap_or(Value::null())
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_store_upvalue(vm_ptr: *mut NyarVM, closure_val: Value, idx: u32, val: Value) {
    let vm = &mut *vm_ptr;
    let closure = closure_val.as_closure();
    if (idx as usize) < closure.upvalues.len() {
        closure.upvalues[idx as usize].set(val);
        val.write_barrier(&vm.gc);
    }
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
        closure.upvalues[idx as usize].get()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_eval(vm_ptr: *mut NyarVM, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let depth = vm.frames.len();
    match vm.execute_eval(argc as u8) {
        Ok(Some(_)) => drive_until(vm, depth),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_capture_cont(vm_ptr: *mut NyarVM) -> Value {
    let vm = &mut *vm_ptr;
    match vm.execute_capture_cont() {
        Ok(_) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_yield(vm_ptr: *mut NyarVM) -> Value {
    let vm = &mut *vm_ptr;
    match vm.execute_yield() {
        Ok(_) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_resume(vm_ptr: *mut NyarVM, cont: Value, val: Value) -> Value {
    let vm = &mut *vm_ptr;
    let depth = vm.frames.len();
    match vm.execute_resume(cont, val) {
        Ok(Some(_)) => drive_until(vm, depth),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_load_global(vm_ptr: *mut NyarVM, name_idx: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let module = &vm.modules[module_idx];
    let name = match module.constants.get(name_idx as usize) {
        Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
        Some(Constant::QualifiedName(qn)) => qn.clone(),
        _ => return Value::null(),
    };
    let symbol = vm.builtins.get(&name).map(|v| Ok(*v)).or_else(|| vm.symbol_table.get(&name).map(|res| Err(*res)));

    match symbol {
        Some(Ok(val)) => {
            vm.push(val).unwrap();
            val
        }
        Some(Err((m_idx, c_idx))) => {
            let val = Value::function(m_idx as usize, c_idx as usize, &vm.gc);
            vm.push(val).unwrap();
            val
        }
        None => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call_symbol(vm_ptr: *mut NyarVM, name_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let depth = vm.frames.len();
    match vm.execute_call_symbol(name_idx as u16, argc as u16, module_idx) {
        Ok(Some(_)) => drive_until(vm, depth),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_invoke_method(vm_ptr: *mut NyarVM, name_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let depth = vm.frames.len();
    match vm.execute_invoke_method(name_idx as u16, argc as u16, module_idx) {
        Ok(Some(_)) => drive_until(vm, depth),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_store_global(vm_ptr: *mut NyarVM, name_idx: u32, val: Value) {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let module = &vm.modules[module_idx];
    let name = match module.constants.get(name_idx as usize) {
        Some(Constant::QualifiedName(qn)) => qn.clone(),
        Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
        _ => return,
    };
    vm.builtins.insert(name, val);
    val.write_barrier(&vm.gc);
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call(vm_ptr: *mut NyarVM, chunk_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let depth = vm.frames.len();
    match vm.execute_call(chunk_idx as u16, argc as u16, module_idx) {
        Ok(Some(_)) => drive_until(vm, depth),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call_virtual(vm_ptr: *mut NyarVM, name_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let depth = vm.frames.len();
    match vm.execute_call_virtual(name_idx as u16, argc as u8, module_idx) {
        Ok(Some(_)) => drive_until(vm, depth),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_call_dynamic(vm_ptr: *mut NyarVM, name_idx: u32, argc: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let depth = vm.frames.len();
    match vm.execute_call_dynamic(name_idx as u16, argc as u8, module_idx) {
        Ok(Some(_)) => drive_until(vm, depth),
        Ok(None) => vm.pop().unwrap_or(Value::null()),
        Err(_) => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_close_upvalues(vm_ptr: *mut NyarVM) {
    let vm = &mut *vm_ptr;
    vm.execute_close_upvalues().unwrap();
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_new_list(vm_ptr: *mut NyarVM, len: u32) -> Value {
    let vm = &mut *vm_ptr;
    let mut items = Vec::with_capacity(len as usize);
    for _ in 0..len {
        items.push(vm.pop().unwrap());
    }
    items.reverse();
    let list = Value::list(items, &vm.gc);
    vm.push(list).unwrap();
    list
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_list_push_right(vm_ptr: *mut NyarVM, list_val: Value, val: Value) {
    let vm = &mut *vm_ptr;
    let list = list_val.as_list_mut();
    list.items.push(val);
    val.write_barrier(&vm.gc);
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_list_pop_right(_vm_ptr: *mut NyarVM, list_val: Value) -> Value {
    let list = list_val.as_list_mut();
    list.items.pop().unwrap_or(Value::null())
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_list_push_left(vm_ptr: *mut NyarVM, list_val: Value, val: Value) {
    let vm = &mut *vm_ptr;
    let list = list_val.as_list_mut();
    list.items.insert(0, val);
    val.write_barrier(&vm.gc);
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_list_pop_left(_vm_ptr: *mut NyarVM, list_val: Value) -> Value {
    let list = list_val.as_list_mut();
    if list.items.is_empty() {
        Value::null()
    } else {
        list.items.remove(0)
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_has_key(_vm_ptr: *mut NyarVM, obj_val: Value, key_val: Value) -> bool {
    if obj_val.is_dyn_object() {
        let obj = obj_val.as_dyn_object();
        let key = key_val.try_as_str().unwrap_or("");
        obj.entries.contains_key(key)
    } else {
        false
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_remove_key(_vm_ptr: *mut NyarVM, obj_val: Value, key_val: Value) -> Value {
    if obj_val.is_dyn_object() {
        let obj = obj_val.as_dyn_object_mut();
        let key = key_val.try_as_str().unwrap_or("");
        obj.entries.remove(key).unwrap_or(Value::null())
    } else {
        Value::null()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_bytes(vm_ptr: *mut NyarVM, bytes_len: u32) -> Value {
    let vm = &mut *vm_ptr;
    let mut bytes = Vec::with_capacity(bytes_len as usize);
    for _ in 0..bytes_len {
        bytes.push(vm.pop().unwrap().as_int() as u8);
    }
    bytes.reverse();
    use num_bigint::BigInt as NativeBigInt;
    let bi = NativeBigInt::from_bytes_be(num_bigint::Sign::Plus, &bytes);
    Value::bigint(BigInt(bi), &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_const(vm_ptr: *mut NyarVM, sign: i64, bytes_val: Value) -> Value {
    let vm = &mut *vm_ptr;
    let bi_val = bytes_val.as_bigint();
    let mut bi = bi_val.0.clone();
    if sign < 0 {
        bi = -bi;
    }
    Value::bigint(BigInt(bi), &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_add(vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> Value {
    let vm = &mut *vm_ptr;
    let l = lhs.as_bigint();
    let r = rhs.as_bigint();
    Value::bigint(BigInt(&l.0 + &r.0), &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_sub(vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> Value {
    let vm = &mut *vm_ptr;
    let l = lhs.as_bigint();
    let r = rhs.as_bigint();
    Value::bigint(BigInt(&l.0 - &r.0), &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_mul(vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> Value {
    let vm = &mut *vm_ptr;
    let l = lhs.as_bigint();
    let r = rhs.as_bigint();
    Value::bigint(BigInt(&l.0 * &r.0), &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_div(vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> Value {
    let vm = &mut *vm_ptr;
    let l = lhs.as_bigint();
    let r = rhs.as_bigint();
    Value::bigint(BigInt(&l.0 / &r.0), &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_mod(vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> Value {
    let vm = &mut *vm_ptr;
    let l = lhs.as_bigint();
    let r = rhs.as_bigint();
    Value::bigint(BigInt(&l.0 % &r.0), &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_neg(vm_ptr: *mut NyarVM, val: Value) -> Value {
    let vm = &mut *vm_ptr;
    let v = val.as_bigint();
    Value::bigint(BigInt(-v.0.clone()), &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_eq(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.as_bigint().0 == rhs.as_bigint().0
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_ne(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.as_bigint().0 != rhs.as_bigint().0
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_lt(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.as_bigint().0 < rhs.as_bigint().0
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_le(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.as_bigint().0 <= rhs.as_bigint().0
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_gt(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.as_bigint().0 > rhs.as_bigint().0
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_ge(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.as_bigint().0 >= rhs.as_bigint().0
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_to_i64(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
    val.as_bigint().to_i64()
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_from_i64(vm_ptr: *mut NyarVM, val: i64) -> Value {
    let vm = &mut *vm_ptr;
    Value::bigint_from_i64(val, &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_bigint_to_string(vm_ptr: *mut NyarVM, val: Value) -> Value {
    let vm = &mut *vm_ptr;
    let s = val.as_bigint().0.to_string();
    Value::string(s, &vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_len_bytes(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
    val.try_as_str().unwrap_or("").len() as i64
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_len_chars(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
    val.try_as_str().unwrap_or("").chars().count() as i64
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_substr(vm_ptr: *mut NyarVM, s_val: Value, start: i64, len: i64) -> Value {
    let vm = &mut *vm_ptr;
    let s = s_val.try_as_str().unwrap_or("");
    let start = start.max(0) as usize;
    let len = len.max(0) as usize;
    let end = (start + len).min(s.len());
    if start >= s.len() {
        Value::string("".to_string(), &vm.gc)
    } else {
        Value::string(s[start..end].to_string(), &vm.gc)
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_eq(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") == rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_ne(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") != rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_lt(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") < rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_le(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") <= rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_gt(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") > rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_string_ge(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") >= rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_match_variant(_vm_ptr: *mut NyarVM, val: Value, variant_idx: i64) -> bool {
    if let Some(obj) = val.try_as_object() {
        obj.class_idx as i64 == variant_idx
    } else {
        false
    }
}
