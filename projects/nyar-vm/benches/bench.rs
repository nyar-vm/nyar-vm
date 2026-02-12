use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nyar_vm::bytecode::instruction::Instruction;
use nyar_vm::vm::value::Value;
use nyar_vm::NyarVM;

fn bench_dispatch_f64_add(c: &mut Criterion) {
    let mut vm = NyarVM::new();
    c.bench_function("dispatch_f64_add", |b| {
        b.iter(|| {
            vm.push(Value::float(black_box(1.0))).unwrap();
            vm.push(Value::float(black_box(2.0))).unwrap();
            vm.dispatch_instruction(Instruction::F64Add, 0, 0).unwrap();
            black_box(vm.pop().unwrap());
        })
    });
}

fn bench_dispatch_i64_add(c: &mut Criterion) {
    let mut vm = NyarVM::new();
    c.bench_function("dispatch_i64_add", |b| {
        b.iter(|| {
            vm.push(Value::int(black_box(1))).unwrap();
            vm.push(Value::int(black_box(2))).unwrap();
            vm.dispatch_instruction(Instruction::I64Add, 0, 0).unwrap();
            black_box(vm.pop().unwrap());
        })
    });
}

fn bench_dispatch_string_len_bytes(c: &mut Criterion) {
    let mut vm = NyarVM::new();
    c.bench_function("dispatch_string_len_bytes", |b| {
        b.iter(|| {
            let s = Value::string(black_box("hello".to_string()), &vm.gc);
            vm.push(s).unwrap();
            vm.dispatch_instruction(Instruction::StringLenBytes, 0, 0).unwrap();
            black_box(vm.pop().unwrap());
        })
    });
}

criterion_group!(
    benches,
    bench_dispatch_f64_add,
    bench_dispatch_i64_add,
    bench_dispatch_string_len_bytes
);
criterion_main!(benches);
