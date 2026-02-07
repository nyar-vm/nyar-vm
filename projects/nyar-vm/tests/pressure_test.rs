use nyar_vm::bytecode::format::{Chunk, Constant, NyarModule};
use nyar_vm::bytecode::opcode::Opcode;
use nyar_vm::vm::async_rt::VmFuture;
use nyar_vm::vm::core::NyarVM;
use std::time::Instant;

#[tokio::test]
async fn pressure_test_10k_spawn_delay() {
    // Callee closure: calls std.async.delay(10)
    let mut callee = Vec::new();
    callee.push(Opcode::Push as u8);
    callee.extend_from_slice(&0u16.to_le_bytes()); // 10ms
    callee.push(Opcode::FFICall as u8);
    callee.extend_from_slice(&1u16.to_le_bytes()); // "std.async.delay"
    callee.push(1u8); // 1 arg
    callee.push(Opcode::Await as u8);
    callee.push(Opcode::Return as u8);

    // Main: loops 10,000 times to spawn tasks
    let mut main = Vec::new();
    
    // 0: Push 0
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&2u16.to_le_bytes()); 
    // 1: StoreLocal 0 (count)
    main.push(Opcode::StoreLocal as u8);
    main.push(0u8);

    // 2: NewList 0
    main.push(Opcode::NewList as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    // 3: StoreLocal 1 (list)
    main.push(Opcode::StoreLocal as u8);
    main.push(1u8);
    
    // --- loop_start = index 4 ---
    
    // 4: LoadLocal 0
    main.push(Opcode::LoadLocal as u8);
    main.push(0u8);
    // 5: Push 10000
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&3u16.to_le_bytes());
    // 6: I64Ext LtS
    main.push(Opcode::I64Ext as u8);
    main.push(0x12);
    
    // 7: JumpIfFalse to await_loop (index 19)
    // off = 19 - 7 = 12
    main.push(Opcode::JumpIfFalse as u8);
    main.extend_from_slice(&12i16.to_le_bytes());
    
    // 8: MakeClosure 0
    main.push(Opcode::MakeClosure as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(0u8);
    // 9: FFICall spawn
    main.push(Opcode::FFICall as u8);
    main.extend_from_slice(&4u16.to_le_bytes());
    main.push(1u8);
    
    // 10: LoadLocal 1
    main.push(Opcode::LoadLocal as u8);
    main.push(1u8);
    // 11: Swap 1
    main.push(Opcode::Swap as u8);
    main.push(1u8);
    // 12: PushElementRight
    main.push(Opcode::PushElementRight as u8);
    // 13: StoreLocal 1
    main.push(Opcode::StoreLocal as u8);
    main.push(1u8);
    
    // 14: LoadLocal 0
    main.push(Opcode::LoadLocal as u8);
    main.push(0u8);
    // 15: Push 1
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&5u16.to_le_bytes());
    // 16: I64Ext Add
    main.push(Opcode::I64Ext as u8);
    main.push(0x01);
    // 17: StoreLocal 0
    main.push(Opcode::StoreLocal as u8);
    main.push(0u8);
    
    // 18: Jump back to 4
    // off = 4 - 18 = -14
    main.push(Opcode::Jump as u8);
    main.extend_from_slice(&(-14i16).to_le_bytes());
    
    // --- await_loop = index 19 ---
    
    // 19: Push 0
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&2u16.to_le_bytes());
    // 20: StoreLocal 0
    main.push(Opcode::StoreLocal as u8);
    main.push(0u8);
    
    // 21: LoadLocal 0
    main.push(Opcode::LoadLocal as u8);
    main.push(0u8);
    // 22: Push 10000
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&3u16.to_le_bytes());
    // 23: I64Ext LtS
    main.push(Opcode::I64Ext as u8);
    main.push(0x12);
    
    // 24: JumpIfFalse to end (index 35)
    // off = 35 - 24 = 11
    main.push(Opcode::JumpIfFalse as u8);
    main.extend_from_slice(&11i16.to_le_bytes());
    
    // 25: LoadLocal 1
    main.push(Opcode::LoadLocal as u8);
    main.push(1u8);
    // 26: LoadLocal 0
    main.push(Opcode::LoadLocal as u8);
    main.push(0u8);
    // 27: GetElement
    main.push(Opcode::GetElement as u8);
    // 28: Await
    main.push(Opcode::Await as u8);
    // 29: Pop
    main.push(Opcode::Pop as u8);
    
    // 30: LoadLocal 0
    main.push(Opcode::LoadLocal as u8);
    main.push(0u8);
    // 31: Push 1
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&5u16.to_le_bytes());
    // 32: I64Ext Add
    main.push(Opcode::I64Ext as u8);
    main.push(0x01);
    // 33: StoreLocal 0
    main.push(Opcode::StoreLocal as u8);
    main.push(0u8);
    
    // 34: Jump back to 21
    // off = 21 - 34 = -13
    main.push(Opcode::Jump as u8);
    main.extend_from_slice(&(-13i16).to_le_bytes());
    
    // 35: LoadLocal 0
    main.push(Opcode::LoadLocal as u8);
    main.push(0u8);
    // 36: Return
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::Int(10),                 // 0
            Constant::String("std.async.Async.delay".to_string()), // 1
            Constant::Int(0),                  // 2
            Constant::Int(10000),              // 3
            Constant::String("std.async.Async.spawn".to_string()), // 4
            Constant::Int(1),                  // 5
        ],
        chunks: vec![
            Chunk {
                max_stack: 8,
                code: callee,
                ..Default::default()
            },
            Chunk {
                locals: 2,
                max_stack: 16,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let mut vm = NyarVM::new();
    vm.ffi.register_std();
    let module_idx = vm.load_named_module(module, "main".to_string());
    
    let start = Instant::now();
    let vm_future = VmFuture {
        vm: &mut vm,
        module_idx,
        chunk_idx: 1,
    };
    let v = vm_future.await.expect("Execution failed");
    let duration = start.elapsed();
    
    println!("Spawned and awaited 10,000 tasks in {:?}", duration);
    assert_eq!(v.as_int(), 10000);
}
