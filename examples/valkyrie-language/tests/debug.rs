use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::vm::interpreter::NyarVM;
use valkyrie_language::compile_text_to_module;

#[test]
fn assert_true_ok() {
    let module = compile_text_to_module("assert(true)").unwrap();
    let chunk = module.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        module.constants.clone(),
        module.chunks.clone(),
        module.classes.clone(),
        module.traits.clone(),
        module.impls.clone(),
        module.effects.clone(),
    );
    let r = vm.execute(&program);
    assert!(r.is_ok());
}

#[test]
fn assert_false_err() {
    let module = compile_text_to_module("assert(false)").unwrap();
    let chunk = module.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        module.constants.clone(),
        module.chunks.clone(),
        module.classes.clone(),
        module.traits.clone(),
        module.impls.clone(),
        module.effects.clone(),
    );
    let r = vm.execute(&program);
    assert!(r.is_err());
}

#[test]
fn debug_print_ok() {
    let module = compile_text_to_module("debug(1)").unwrap();
    let chunk = module.chunks[0].clone();
    let program = Decoder::new(&chunk.code).decode_all().unwrap();
    let mut vm = NyarVM::new(
        module.constants.clone(),
        module.chunks.clone(),
        module.classes.clone(),
        module.traits.clone(),
        module.impls.clone(),
        module.effects.clone(),
    );
    let r = vm.execute(&program);
    assert!(r.is_ok());
}
