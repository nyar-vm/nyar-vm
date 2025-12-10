use criterion::{criterion_group, criterion_main, Criterion};
use nyar_vm::bytecode::opcode::Opcode;
use nyar_vm::bytecode::format::{minimal_module_with_chunk, Constant};
use nyar_vm::vm::interpreter::VM;

fn bench_push_return(c: &mut Criterion) {
    let mut code = Vec::new();
    code.push(Opcode::PushConst as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![Constant::Int(1)]);
    let mut vm = VM::new(module.constants.clone(), module.effects.clone());
    let chunk = module.chunks[0].clone();
    c.bench_function("push_return", |b| {
        b.iter(|| {
            let _ = vm.execute(&chunk);
        })
    });
}

criterion_group!(benches, bench_push_return);
criterion_main!(benches);

