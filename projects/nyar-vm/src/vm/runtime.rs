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
pub unsafe extern "win64" fn nyar_vm_get_element(vm_ptr: *mut NyarVM, obj: Value, idx: Value) -> Value {
    let _vm = &mut *vm_ptr;
    match obj {
        Value::List(l) => {
            let i = idx.as_int() as usize;
            l.items.get(i).cloned().unwrap_or(Value::null())
        }
        _ => Value::null(),
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_set_element(vm_ptr: *mut NyarVM, obj: Value, idx: Value, val: Value) {
    let vm = &mut *vm_ptr;
    match obj {
        Value::List(mut l) => {
            let i = idx.as_int() as usize;
            if i < l.items.len() {
                l.items[i] = val;
                val.write_barrier(&vm.gc);
            }
        }
        Value::Array(mut a) => {
            let i = idx.as_int() as usize;
            if i < a.items.len() {
                a.items[i] = val;
                val.write_barrier(&vm.gc);
            }
        }
        Value::DynObject(mut o) => {
            let key = idx.try_as_str().unwrap_or("");
            o.entries.insert(key.to_string(), val);
            val.write_barrier(&vm.gc);
        }
        _ => {}
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_new_object(vm_ptr: *mut NyarVM, class_idx: u32) -> Value {
    let vm = &mut *vm_ptr;
    let module_idx = vm.frames.last().unwrap().module_idx;
    let class_info = &vm.modules[module_idx].classes[class_idx as usize];
    let fields_count = class_info.fields.len();
    let mut fields = Vec::with_capacity(fields_count);
    for _ in 0..fields_count {
        fields.push(vm.pop().unwrap());
    }
    fields.reverse();
    let obj = Value::object(class_idx as u16, fields, &vm.gc);
    vm.push(obj).unwrap();
    obj
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
    val.get_type() as i64
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_size_of(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
    match val {
        Value::List(l) => l.items.len() as i64,
        Value::String(s) => s.len() as i64,
        Value::DynObject(o) => o.entries.len() as i64,
        _ => 0,
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
pub unsafe extern "win64" fn nyar_vm_load_upvalue(_vm_ptr: *mut NyarVM, closure_val: Value, idx: u32) -> Value {
    let closure = closure_val.as_closure();
    if (idx as usize) < closure.upvalues.len() {
        closure.upvalues[idx as usize].0
    } else {
        Value::null()
    }
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_store_upvalue(vm_ptr: *mut NyarVM, closure_val: Value, idx: u32, val: Value) {
    let vm = &mut *vm_ptr;
    let closure = closure_val.as_closure_mut();
    if (idx as usize) < closure.upvalues.len() {
        closure.upvalues[idx as usize].0 = val;
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
pub unsafe extern "win64" fn nyar_vm_str_len_bytes(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
    val.try_as_str().unwrap_or("").len() as i64
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_str_len_chars(_vm_ptr: *mut NyarVM, val: Value) -> i64 {
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
pub unsafe extern "win64" fn nyar_vm_str_lt(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") < rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_str_le(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") <= rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_str_gt(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") > rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_str_ge(_vm_ptr: *mut NyarVM, lhs: Value, rhs: Value) -> bool {
    lhs.try_as_str().unwrap_or("") >= rhs.try_as_str().unwrap_or("")
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_new_dyn_object(vm_ptr: *mut NyarVM) -> Value {
    let vm = &mut *vm_ptr;
    Value::dyn_object(&vm.gc)
}

#[no_mangle]
pub unsafe extern "win64" fn nyar_vm_match_variant(_vm_ptr: *mut NyarVM, val: Value, variant_idx: i64) -> bool {
    if let Some(obj) = val.try_as_object() {
        obj.class_idx as i64 == variant_idx
    } else {
        false
    }
}
