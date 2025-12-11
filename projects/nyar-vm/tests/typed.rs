use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::bytecode::format::{minimal_module_with_chunk, NyarModule};
use nyar_vm::bytecode::opcode::{BigIntExt, F32Ext, F64Ext, I32Ext, I64Ext, Opcode, StringExt};
use nyar_vm::vm::interpreter::NyarVM;

#[test]
fn run_i32_add_return() {
    let mut code = Vec::new();
    code.push(Opcode::I32Ext as u8);
    code.push(I32Ext::Const as u8);
    code.extend_from_slice(&42i32.to_le_bytes());
    code.push(Opcode::I32Ext as u8);
    code.push(I32Ext::Const as u8);
    code.extend_from_slice(&0i32.to_le_bytes());
    code.push(Opcode::I32Ext as u8);
    code.push(I32Ext::Add as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_int(), 42);
    }
}

#[test]
fn run_i64_add_return() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&42i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Add as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_int(), 43);
    }
}

#[test]
fn run_f32_add_return() {
    let mut code = Vec::new();
    code.push(Opcode::F32Ext as u8);
    code.push(F32Ext::Const as u8);
    code.extend_from_slice(&(1.5f32).to_bits().to_le_bytes());
    code.push(Opcode::F32Ext as u8);
    code.push(F32Ext::Const as u8);
    code.extend_from_slice(&(2.25f32).to_bits().to_le_bytes());
    code.push(Opcode::F32Ext as u8);
    code.push(F32Ext::Add as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert!((v.as_float() - 3.75).abs() < 1e-6);
    }
}

#[test]
fn run_f64_add_return() {
    let mut code = Vec::new();
    code.push(Opcode::F64Ext as u8);
    code.push(F64Ext::Const as u8);
    code.extend_from_slice(&(1.5f64).to_le_bytes());
    code.push(Opcode::F64Ext as u8);
    code.push(F64Ext::Const as u8);
    code.extend_from_slice(&(2.25f64).to_le_bytes());
    code.push(Opcode::F64Ext as u8);
    code.push(F64Ext::Add as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert!((v.as_float() - 3.75).abs() < 1e-12);
    }
}

#[test]
fn run_i32_unsigned_cmp() {
    let mut code = Vec::new();
    code.push(Opcode::I32Ext as u8);
    code.push(I32Ext::Const as u8);
    code.extend_from_slice(&(-1i32).to_le_bytes());
    code.push(Opcode::I32Ext as u8);
    code.push(I32Ext::Const as u8);
    code.extend_from_slice(&1i32.to_le_bytes());
    code.push(Opcode::I32Ext as u8);
    code.push(I32Ext::LtU as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_bool(), false);
    }
}

#[test]
fn run_i32_to_f64s() {
    let mut code = Vec::new();
    code.push(Opcode::I32Ext as u8);
    code.push(I32Ext::Const as u8);
    code.extend_from_slice(&(-2i32).to_le_bytes());
    code.push(Opcode::I32Ext as u8);
    code.push(I32Ext::ToF64S as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert!((v.as_float() + 2.0).abs() < 1e-12);
    }
}

#[test]
fn run_f64_to_i32u() {
    let mut code = Vec::new();
    code.push(Opcode::F64Ext as u8);
    code.push(F64Ext::Const as u8);
    code.extend_from_slice(&(4294967295.0f64).to_le_bytes());
    code.push(Opcode::F64Ext as u8);
    code.push(F64Ext::ToI32U as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_int(), 4294967295i64);
    }
}

#[test]
fn run_string_concat_return() {
    let mut code = Vec::new();
    code.push(Opcode::StringExt as u8);
    code.push(StringExt::Const as u8);
    code.push(1u8);
    code.extend_from_slice(b"a");
    code.push(Opcode::StringExt as u8);
    code.push(StringExt::Const as u8);
    code.push(1u8);
    code.extend_from_slice(b"b");
    code.push(Opcode::StringExt as u8);
    code.push(StringExt::Concat as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_string(), &"ab".to_string());
    }
}

#[test]
fn run_bigint_add_return() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&42i64.to_le_bytes());
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::FromI64 as u8);
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::FromI64 as u8);
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::Add as u8);
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::ToI64 as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_int(), 43);
    }
}

#[test]
fn run_bigint_cmp_lt_ge() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&(-5i64).to_le_bytes());
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::FromI64 as u8);
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&2i64.to_le_bytes());
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::FromI64 as u8);
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::Lt as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code.clone(), vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_bool(), true);
    }

    let mut code2 = Vec::new();
    code2.push(Opcode::I64Ext as u8);
    code2.push(I64Ext::Const as u8);
    code2.extend_from_slice(&2i64.to_le_bytes());
    code2.push(Opcode::BigIntExt as u8);
    code2.push(BigIntExt::FromI64 as u8);
    code2.push(Opcode::I64Ext as u8);
    code2.push(I64Ext::Const as u8);
    code2.extend_from_slice(&(-5i64).to_le_bytes());
    code2.push(Opcode::BigIntExt as u8);
    code2.push(BigIntExt::FromI64 as u8);
    code2.push(Opcode::BigIntExt as u8);
    code2.push(BigIntExt::Ge as u8);
    code2.push(Opcode::Return as u8);
    let module2 = minimal_module_with_chunk(code2, vec![]);
    let data2 = module2.encode();
    let parsed2 = NyarModule::parse(&data2).unwrap();
    let chunk2 = parsed2.chunks[0].clone();
    let program2 = Decoder::new(&chunk2.code).decode_all().unwrap();
    let mut vm2 = NyarVM::new(
        parsed2.constants,
        parsed2.chunks.clone(),
        parsed2.classes,
        parsed2.traits,
        parsed2.impls,
        parsed2.effects,
    );
    let v2 = vm2.execute(&program2).unwrap();
    unsafe {
        assert_eq!(v2.as_bool(), true);
    }
}

#[test]
fn run_bigint_mod_to_i64() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&10i64.to_le_bytes());
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::FromI64 as u8);
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&3i64.to_le_bytes());
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::FromI64 as u8);
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::Mod as u8);
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::ToI64 as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_int(), 1i64);
    }
}

#[test]
fn run_bigint_to_string() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(I64Ext::Const as u8);
    code.extend_from_slice(&(-123i64).to_le_bytes());
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::FromI64 as u8);
    code.push(Opcode::BigIntExt as u8);
    code.push(BigIntExt::ToString as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let data = module.encode();
    let parsed = NyarModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        parsed.constants,
        parsed.chunks.clone(),
        parsed.classes,
        parsed.traits,
        parsed.impls,
        parsed.effects,
    );
    let v = vm.execute(&program).unwrap();
    unsafe {
        assert_eq!(v.as_string(), &"-123".to_string());
    }
}
