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
    callee.extend_from_slice(&1u16.to_le_bytes()); // "std::async::Async::delay"
    callee.push(1u8); // 1 arg
    callee.push(Opcode::Await as u8);
    callee.push(Opcode::Return as u8);

    // Main: loops 10,000 times to spawn tasks
    let mut main = Vec::new();
    let mut instrs_count = 0;
    let mut instr_indices = Vec::new();

    // 0: Push 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 2, 0]); 
    instrs_count += 1;
    // 1: StoreLocal 0 (count)
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 0]);
    instrs_count += 1;
    // 2: NewList 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::NewList as u8, 0, 0]);
    instrs_count += 1;
    // 3: StoreLocal 1 (list)
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 1]);
    instrs_count += 1;
    
    // --- loop_start = index 4 ---
    let loop_start_idx = instrs_count;
    // 4: LoadLocal 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    // 5: Push 10000
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 3, 0]);
    instrs_count += 1;
    // 6: I64Ext LtS
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x12]);
    instrs_count += 1;
    
    // 7: JumpIfFalse to await_loop (placeholder)
    let jump_to_await_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::JumpIfFalse as u8, 0, 0]);
    instrs_count += 1;
    
    // 8: MakeClosure 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::MakeClosure as u8, 0, 0, 0]);
    instrs_count += 1;
    // 9: FFICall spawn
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::FFICall as u8, 4, 0, 1]);
    instrs_count += 1;
    
    // 10: LoadLocal 1
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 1]);
    instrs_count += 1;
    // 11: Swap 1
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Swap as u8, 1]);
    instrs_count += 1;
    // 12: PushElementRight
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::PushElementRight as u8]);
    instrs_count += 1;
    // 13: StoreLocal 1
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 1]);
    instrs_count += 1;
    
    // 14: LoadLocal 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    // 14: Push 1
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 5, 0]);
    instrs_count += 1;
    // 16: I64Ext Add
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x01]);
    instrs_count += 1;
    // 17: StoreLocal 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 0]);
    instrs_count += 1;
    
    // 18: Jump back to 4
    let jump_back_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Jump as u8, 0, 0]);
    instrs_count += 1;
    let off_back = (loop_start_idx as i16) - (jump_back_idx as i16);
    main[instr_indices[jump_back_idx] + 1..instr_indices[jump_back_idx] + 3].copy_from_slice(&off_back.to_le_bytes());
    
    // --- await_loop = index 19 ---
    let await_loop_start_idx = instrs_count;
    let off_to_await = (await_loop_start_idx as i16) - (jump_to_await_idx as i16);
    main[instr_indices[jump_to_await_idx] + 1..instr_indices[jump_to_await_idx] + 3].copy_from_slice(&off_to_await.to_le_bytes());

    // 19: Push 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 2, 0]);
    instrs_count += 1;
    // 20: StoreLocal 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 0]);
    instrs_count += 1;
    
    // 21: LoadLocal 0
    let await_cond_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    // 22: Push 10000
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 3, 0]);
    instrs_count += 1;
    // 23: I64Ext LtS
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x12]);
    instrs_count += 1;
    
    // 24: JumpIfFalse to end (placeholder)
    let jump_to_end_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::JumpIfFalse as u8, 0, 0]);
    instrs_count += 1;
    
    // 25: LoadLocal 1
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 1]);
    instrs_count += 1;
    // 26: LoadLocal 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    // 27: GetElement
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::GetElement as u8]);
    instrs_count += 1;
    // 28: Await
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Await as u8]);
    instrs_count += 1;
    // 29: Pop
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Pop as u8]);
    instrs_count += 1;
    
    // 30: LoadLocal 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    // 31: Push 1
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 5, 0]);
    instrs_count += 1;
    // 32: I64Ext Add
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x01]);
    instrs_count += 1;
    // 33: StoreLocal 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 0]);
    instrs_count += 1;
    
    // 34: Jump back to 21
    let jump_back_await_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Jump as u8, 0, 0]);
    instrs_count += 1;
    let off_back_await = (await_cond_idx as i16) - (jump_back_await_idx as i16);
    main[instr_indices[jump_back_await_idx] + 1..instr_indices[jump_back_await_idx] + 3].copy_from_slice(&off_back_await.to_le_bytes());
    
    // 35: LoadLocal 0
    let end_idx = instrs_count;
    let off_to_end = (end_idx as i16) - (jump_to_end_idx as i16);
    main[instr_indices[jump_to_end_idx] + 1..instr_indices[jump_to_end_idx] + 3].copy_from_slice(&off_to_end.to_le_bytes());

    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    // 36: Return
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Return as u8]);
    instrs_count += 1;

    let module = NyarModule {
        constants: vec![
            Constant::Int(10),                 // 0
            Constant::String("std::async::Async::delay".to_string()), // 1
            Constant::Int(0),                  // 2
            Constant::Int(10000),              // 3
            Constant::String("std::async::Async::spawn".to_string()), // 4
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
    
    let (allocated, collections) = vm.gc_stats();
    println!("Spawned and awaited 10,000 tasks in {:?}", duration);
    println!("GC Stats: {} bytes allocated, {} collections", allocated, collections);
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
    callee_delay.extend_from_slice(&1u16.to_le_bytes()); // index 1: "std::async::Async::delay"
    callee_delay.push(1u8); // 1 arg
    callee_delay.push(Opcode::Await as u8);
    callee_delay.push(Opcode::Return as u8);

    // Callee 1: calls std.http.get(url)
    let mut callee_http = Vec::new();
    callee_http.push(Opcode::Push as u8);
    callee_http.extend_from_slice(&6u16.to_le_bytes()); // index 6: url
    callee_http.push(Opcode::FFICall as u8);
    callee_http.extend_from_slice(&7u16.to_le_bytes()); // index 7: "std::http::get"
    callee_http.push(1u8); // 1 arg
    callee_http.push(Opcode::Await as u8);
    callee_http.push(Opcode::Return as u8);

    // Main: loops 10,000 times to spawn tasks
    let mut main = Vec::new();
    let mut instrs_count = 0;
    let mut instr_indices = Vec::new();

    // init count=0, list=[]
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 2, 0]); // c_0
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::NewList as u8, 0, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 1]);
    instrs_count += 1;

    let loop_start_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 3, 0]); // c_10000
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x12]); // LtS
    instrs_count += 1;
    let jump_to_await_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::JumpIfFalse as u8, 0, 0]);
    instrs_count += 1;

    // if count % 100 == 0
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 8, 0]); // c_100
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x06]); // RemS
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 2, 0]); // c_0
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x10]); // Eq
    instrs_count += 1;
    let jump_to_delay_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::JumpIfFalse as u8, 0, 0]);
    instrs_count += 1;

    // spawn http
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::MakeClosure as u8, 1, 0, 0]); // chunk 1, 0 upvalues
    instrs_count += 1;
    let jump_to_common_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Jump as u8, 0, 0]);
    instrs_count += 1;

    // spawn delay
    let delay_start_idx = instrs_count;
    let off_to_delay = (delay_start_idx as i16) - (jump_to_delay_idx as i16);
    main[instr_indices[jump_to_delay_idx] + 1..instr_indices[jump_to_delay_idx] + 3].copy_from_slice(&off_to_delay.to_le_bytes());
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::MakeClosure as u8, 0, 0, 0]); // chunk 0, 0 upvalues
    instrs_count += 1;

    // common spawn
    let common_start_idx = instrs_count;
    let off_to_common = (common_start_idx as i16) - (jump_to_common_idx as i16);
    main[instr_indices[jump_to_common_idx] + 1..instr_indices[jump_to_common_idx] + 3].copy_from_slice(&off_to_common.to_le_bytes());
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::FFICall as u8, 4, 0, 1]); // c_spawn, 1 arg
    instrs_count += 1;
    
    // push to list
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 1]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Swap as u8, 1]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::PushElementRight as u8]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 1]);
    instrs_count += 1;

    // increment
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 5, 0]); // c_1
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x01]); // Add
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 0]);
    instrs_count += 1;
    
    let jump_to_start_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Jump as u8, 0, 0]);
    instrs_count += 1;
    let off_to_start = (loop_start_idx as i16) - (jump_to_start_idx as i16);
    main[instr_indices[jump_to_start_idx] + 1..instr_indices[jump_to_start_idx] + 3].copy_from_slice(&off_to_start.to_le_bytes());

    // await loop
    let await_start_idx = instrs_count;
    let off_to_await = (await_start_idx as i16) - (jump_to_await_idx as i16);
    main[instr_indices[jump_to_await_idx] + 1..instr_indices[jump_to_await_idx] + 3].copy_from_slice(&off_to_await.to_le_bytes());

    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 2, 0]); // c_0
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 0]);
    instrs_count += 1;

    let await_loop_start_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 3, 0]); // c_10000
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x12]); // LtS
    instrs_count += 1;
    let jump_to_end_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::JumpIfFalse as u8, 0, 0]);
    instrs_count += 1;

    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 1]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::GetElement as u8]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Await as u8]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Pop as u8]);
    instrs_count += 1;

    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Push as u8, 5, 0]); // c_1
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::I64Ext as u8, 0x01]); // Add
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::StoreLocal as u8, 0]);
    instrs_count += 1;

    let jump_to_await_loop_idx = instrs_count;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Jump as u8, 0, 0]);
    instrs_count += 1;
    let off_to_await_loop = (await_loop_start_idx as i16) - (jump_to_await_loop_idx as i16);
    main[instr_indices[jump_to_await_loop_idx] + 1..instr_indices[jump_to_await_loop_idx] + 3].copy_from_slice(&off_to_await_loop.to_le_bytes());

    let end_idx = instrs_count;
    let off_to_end = (end_idx as i16) - (jump_to_end_idx as i16);
    main[instr_indices[jump_to_end_idx] + 1..instr_indices[jump_to_end_idx] + 3].copy_from_slice(&off_to_end.to_le_bytes());

    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::LoadLocal as u8, 0]);
    instrs_count += 1;
    instr_indices.push(main.len());
    main.extend_from_slice(&[Opcode::Return as u8]);
    instrs_count += 1;

    let module = NyarModule {
        constants: vec![
            Constant::Int(10),                             // 0
            Constant::String("std::async::Async::delay".to_string()), // 1
            Constant::Int(0),                              // 2
            Constant::Int(10000),                          // 3
            Constant::String("std::async::Async::spawn".to_string()), // 4
            Constant::Int(1),                              // 5
            Constant::String(url),                         // 6
            Constant::String("std::http::get".to_string()),  // 7
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
    
    let (allocated, collections) = vm.gc_stats();
    println!("Mixed 10,000 tasks (delay + http) finished in {:?}", duration);
    println!("GC Stats: {} bytes allocated, {} collections", allocated, collections);
    assert_eq!(v.as_int(), 10000);
}
