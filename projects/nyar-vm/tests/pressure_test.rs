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

#[tokio::test]
async fn pressure_test_10k_mixed_delay_http() {
    // Start a simple local HTTP server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                    use tokio::io::AsyncWriteExt;
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        }
    });

    // Callee 0: calls std.async.Async.delay(10)
    let mut callee_delay = Vec::new();
    callee_delay.push(Opcode::Push as u8);
    callee_delay.extend_from_slice(&0u16.to_le_bytes()); // index 0: 10ms
    callee_delay.push(Opcode::FFICall as u8);
    callee_delay.extend_from_slice(&1u16.to_le_bytes()); // index 1: "std.async.Async.delay"
    callee_delay.push(1u8); // 1 arg
    callee_delay.push(Opcode::Await as u8);
    callee_delay.push(Opcode::Return as u8);

    // Callee 1: calls std.http.get(url)
    let mut callee_http = Vec::new();
    callee_http.push(Opcode::Push as u8);
    callee_http.extend_from_slice(&6u16.to_le_bytes()); // index 6: url
    callee_http.push(Opcode::FFICall as u8);
    callee_http.extend_from_slice(&7u16.to_le_bytes()); // index 7: "std.http.get"
    callee_http.push(1u8); // 1 arg
    callee_http.push(Opcode::Await as u8);
    callee_http.push(Opcode::Return as u8);

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
    
    // 7: JumpIfFalse to await_loop (index 27)
    // off = 27 - 7 = 20
    main.push(Opcode::JumpIfFalse as u8);
    main.extend_from_slice(&20i16.to_le_bytes());

    // --- Decision: if count % 100 == 0 spawn HTTP, else delay ---
    // 8: LoadLocal 0
    main.push(Opcode::LoadLocal as u8);
    main.push(0u8);
    // 9: Push 100
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&8u16.to_le_bytes());
    // 10: I64Ext Mod
    main.push(Opcode::I64Ext as u8);
    main.push(0x06); // Mod
    // 11: Push 0
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&2u16.to_le_bytes());
    // 12: I64Ext Eq
    main.push(Opcode::I64Ext as u8);
    main.push(0x0E); // Eq
    // 13: JumpIfFalse to delay (index 18)
    // off = 18 - 13 = 5
    main.push(Opcode::JumpIfFalse as u8);
    main.extend_from_slice(&5i16.to_le_bytes());

    // --- Spawn HTTP (index 14) ---
    // 14: MakeClosure 1 (callee_http)
    main.push(Opcode::MakeClosure as u8);
    main.extend_from_slice(&1u16.to_le_bytes());
    main.push(0u8);
    // 15: Jump to common_spawn (index 20)
    // off = 20 - 15 = 5
    main.push(Opcode::Jump as u8);
    main.extend_from_slice(&5i16.to_le_bytes());

    // --- Spawn Delay (index 18) ---
    // 18: MakeClosure 0 (callee_delay)
    main.push(Opcode::MakeClosure as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(0u8);

    // --- Common Spawn (index 20) ---
    // 20: FFICall spawn
    main.push(Opcode::FFICall as u8);
    main.extend_from_slice(&4u16.to_le_bytes());
    main.push(1u8);
    
    // 21: LoadLocal 1
    main.push(Opcode::LoadLocal as u8);
    main.push(1u8);
    // 22: Swap 1
    main.push(Opcode::Swap as u8);
    main.push(1u8);
    // 23: PushElementRight
    main.push(Opcode::PushElementRight as u8);
    // 24: StoreLocal 1
    main.push(Opcode::StoreLocal as u8);
    main.push(1u8);
    
    // 25: LoadLocal 0
    main.push(Opcode::LoadLocal as u8);
    main.push(0u8);
    // 26: Push 1
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&5u16.to_le_bytes());
    // 27: I64Ext Add
    main.push(Opcode::I64Ext as u8);
    main.push(0x01);
    // 28: StoreLocal 0
    main.push(Opcode::StoreLocal as u8);
    main.push(0u8);
    
    // 29: Jump back to 4
    // off = 4 - 29 = -25
    main.push(Opcode::Jump as u8);
    main.extend_from_slice(&(-25i16).to_le_bytes());
    
    // --- await_loop = index 30 --- (fixed indices)
    // Wait, I messed up the indices above because of the inserted decision logic.
    // Let's recalculate from index 7 jump.
    // 7: JumpIfFalse to await_loop (off = 23, so index 7+1+23 = 31?)
    // Actually let's just use a more robust way to generate bytecode in the test if I can,
    // but manually calculating is fine if I'm careful.
    
    // Recalculating:
    // 4: LoadLocal 0 (2 bytes)
    // 5: Push 10000 (3 bytes)
    // 6: I64Ext LtS (2 bytes)
    // 7: JumpIfFalse to await_loop (3 bytes) -> Total so far: 10 bytes from start of main
    // 8: LoadLocal 0 (2 bytes)
    // 9: Push 100 (3 bytes)
    // 10: I64Ext Mod (2 bytes)
    // 11: Push 0 (3 bytes)
    // 12: I64Ext Eq (2 bytes)
    // 13: JumpIfFalse to delay (3 bytes) -> Total: 10 + 15 = 25 bytes
    // 14: MakeClosure 1 (4 bytes)
    // 15: Jump to common (3 bytes) -> Total: 25 + 7 = 32 bytes
    // 18: MakeClosure 0 (4 bytes) -> Total: 32 + 4 = 36 bytes
    // 20: FFICall spawn (4 bytes)
    // 21: LoadLocal 1 (2 bytes)
    // 22: Swap 1 (2 bytes)
    // 23: PushElementRight (1 byte)
    // 24: StoreLocal 1 (2 bytes) -> Total: 36 + 11 = 47 bytes
    // 25: LoadLocal 0 (2 bytes)
    // 26: Push 1 (3 bytes)
    // 27: I64Ext Add (2 bytes)
    // 28: StoreLocal 0 (2 bytes) -> Total: 47 + 9 = 56 bytes
    // 29: Jump to 4 (3 bytes) -> Total: 56 + 3 = 59 bytes
    
    // So await_loop starts at 59.
    // Jump at 7 (pos 10) to 59: off = 59 - (10+3) = 46.
    
    // Re-writing the bytecode generation more cleanly:
    let mut main = Vec::new();
    let c_0 = 2u16;
    let c_10000 = 3u16;
    let c_spawn = 4u16;
    let c_1 = 5u16;
    let c_100 = 8u16;

    // init count=0, list=[]
    main.push(Opcode::Push as u8); main.extend_from_slice(&c_0.to_le_bytes());
    main.push(Opcode::StoreLocal as u8); main.push(0);
    main.push(Opcode::NewList as u8); main.extend_from_slice(&0u16.to_le_bytes());
    main.push(Opcode::StoreLocal as u8); main.push(1);

    let loop_start = main.len();
    main.push(Opcode::LoadLocal as u8); main.push(0);
    main.push(Opcode::Push as u8); main.extend_from_slice(&c_10000.to_le_bytes());
    main.push(Opcode::I64Ext as u8); main.push(0x12); // LtS
    let jump_to_await_pos = main.len();
    main.push(Opcode::JumpIfFalse as u8); main.extend_from_slice(&0i16.to_le_bytes());

    // if count % 100 == 0
    main.push(Opcode::LoadLocal as u8); main.push(0);
    main.push(Opcode::Push as u8); main.extend_from_slice(&c_100.to_le_bytes());
    main.push(Opcode::I64Ext as u8); main.push(0x06); // RemS
    main.push(Opcode::Push as u8); main.extend_from_slice(&c_0.to_le_bytes());
    main.push(Opcode::I64Ext as u8); main.push(0x10); // Eq
    let jump_to_delay_pos = main.len();
    main.push(Opcode::JumpIfFalse as u8); main.extend_from_slice(&0i16.to_le_bytes());

    // spawn http
    main.push(Opcode::MakeClosure as u8); main.extend_from_slice(&1u16.to_le_bytes()); main.push(0);
    let jump_to_common_pos = main.len();
    main.push(Opcode::Jump as u8); main.extend_from_slice(&0i16.to_le_bytes());

    // spawn delay
    let delay_start = main.len();
    let off_to_delay = (delay_start - (jump_to_delay_pos + 3)) as i16;
    main[jump_to_delay_pos+1..jump_to_delay_pos+3].copy_from_slice(&off_to_delay.to_le_bytes());
    main.push(Opcode::MakeClosure as u8); main.extend_from_slice(&0u16.to_le_bytes()); main.push(0);

    // common spawn
    let common_start = main.len();
    let off_to_common = (common_start - (jump_to_common_pos + 3)) as i16;
    main[jump_to_common_pos+1..jump_to_common_pos+3].copy_from_slice(&off_to_common.to_le_bytes());
    main.push(Opcode::FFICall as u8); main.extend_from_slice(&c_spawn.to_le_bytes()); main.push(1);
    
    // push to list
    main.push(Opcode::LoadLocal as u8); main.push(1);
    main.push(Opcode::Swap as u8); main.push(1);
    main.push(Opcode::PushElementRight as u8);
    main.push(Opcode::StoreLocal as u8); main.push(1);

    // increment
    main.push(Opcode::LoadLocal as u8); main.push(0);
    main.push(Opcode::Push as u8); main.extend_from_slice(&c_1.to_le_bytes());
    main.push(Opcode::I64Ext as u8); main.push(0x01); // Add
    main.push(Opcode::StoreLocal as u8); main.push(0);
    
    let off_to_start = (loop_start as i32 - (main.len() as i32 + 3)) as i16;
    main.push(Opcode::Jump as u8); main.extend_from_slice(&off_to_start.to_le_bytes());

    // await loop
    let await_start = main.len();
    let off_to_await = (await_start - (jump_to_await_pos + 3)) as i16;
    main[jump_to_await_pos+1..jump_to_await_pos+3].copy_from_slice(&off_to_await.to_le_bytes());

    main.push(Opcode::Push as u8); main.extend_from_slice(&c_0.to_le_bytes());
    main.push(Opcode::StoreLocal as u8); main.push(0);

    let await_loop_start = main.len();
    main.push(Opcode::LoadLocal as u8); main.push(0);
    main.push(Opcode::Push as u8); main.extend_from_slice(&c_10000.to_le_bytes());
    main.push(Opcode::I64Ext as u8); main.push(0x12); // LtS
    let jump_to_end_pos = main.len();
    main.push(Opcode::JumpIfFalse as u8); main.extend_from_slice(&0i16.to_le_bytes());

    main.push(Opcode::LoadLocal as u8); main.push(1);
    main.push(Opcode::LoadLocal as u8); main.push(0);
    main.push(Opcode::GetElement as u8);
    main.push(Opcode::Await as u8);
    main.push(Opcode::Pop as u8);

    main.push(Opcode::LoadLocal as u8); main.push(0);
    main.push(Opcode::Push as u8); main.extend_from_slice(&c_1.to_le_bytes());
    main.push(Opcode::I64Ext as u8); main.push(0x01); // Add
    main.push(Opcode::StoreLocal as u8); main.push(0);

    let off_to_await_loop = (await_loop_start as i32 - (main.len() as i32 + 3)) as i16;
    main.push(Opcode::Jump as u8); main.extend_from_slice(&off_to_await_loop.to_le_bytes());

    let end_start = main.len();
    let off_to_end = (end_start - (jump_to_end_pos + 3)) as i16;
    main[jump_to_end_pos+1..jump_to_end_pos+3].copy_from_slice(&off_to_end.to_le_bytes());

    main.push(Opcode::LoadLocal as u8); main.push(0);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::Int(10),                             // 0
            Constant::String("std.async.Async.delay".to_string()), // 1
            Constant::Int(0),                              // 2
            Constant::Int(10000),                          // 3
            Constant::String("std.async.Async.spawn".to_string()), // 4
            Constant::Int(1),                              // 5
            Constant::String(url),                         // 6
            Constant::String("std.http.get".to_string()),  // 7
            Constant::Int(100),                            // 8
        ],
        chunks: vec![
            Chunk {
                max_stack: 8,
                code: callee_delay,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: callee_http,
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
        chunk_idx: 2,
    };
    let v = vm_future.await.expect("Execution failed");
    let duration = start.elapsed();
    
    println!("Spawned and awaited 10,000 mixed tasks in {:?}", duration);
    assert_eq!(v.as_int(), 10000);
}
