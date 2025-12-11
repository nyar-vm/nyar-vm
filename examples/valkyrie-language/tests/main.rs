use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use valkyrie_language::compile_text_to_module;
use nyar_vm::vm::interpreter::NyarVM;
use nyar_vm::bytecode::decoder::Decoder;

#[test]
fn test_all_vk_files() {
    let tests_dir = Path::new("tests");
    let entries = fs::read_dir(tests_dir).expect("Failed to read tests directory");

    for entry in entries {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "vk") {
            println!("Running test: {:?}", path);
            run_test(&path);
        }
    }
}

fn run_test(path: &Path) {
    let src = fs::read_to_string(path).expect("Failed to read file");
    
    // Parse expected output
    let mut expected_output = Vec::new();
    for line in src.lines() {
        if let Some(idx) = line.find("# Expect: ") {
            let expect = line[idx + 10..].trim().to_string();
            expected_output.push(expect);
        }
    }

    if expected_output.is_empty() {
        println!("Skipping verification for {:?} (no expectations found)", path);
        // Still verify it compiles
        compile_text_to_module(&src).expect("Failed to compile");
        return;
    }

    let module = compile_text_to_module(&src).expect("Failed to compile");
    
    // Setup VM
    let mut vm = NyarVM::new(
        module.constants,
        module.chunks.clone(), // Clone chunks because VM takes ownership (actually VM takes Vec<Chunk>, module has Vec<Chunk>)
        module.classes,
        module.traits,
        module.impls,
        module.effects,
    );

    let output_buffer = Rc::new(RefCell::new(Vec::new()));
    let output_clone = output_buffer.clone();
    vm.stdout = Some(Box::new(move |msg: &str| {
        output_clone.borrow_mut().push(msg.to_string());
    }));

    // Find main chunk (index 0)
    let main_chunk = &module.chunks[0];
    let decoder = Decoder::new(&main_chunk.code);
    let instrs = decoder.decode_all().expect("Failed to decode main chunk");
    
    vm.execute(&instrs).expect("VM execution failed");

    let actual_output = output_buffer.borrow();
    
    // Check if actual output matches expected output
    // Note: Test files might have expectations interleaved, but we collect them all.
    // The order should match the execution order.
    
    if actual_output.len() != expected_output.len() {
        println!("Expected: {:?}", expected_output);
        println!("Actual:   {:?}", actual_output);
        panic!("Output line count mismatch for {:?}", path);
    }
    
    for (i, (actual, expected)) in actual_output.iter().zip(expected_output.iter()).enumerate() {
        assert_eq!(actual, expected, "Output mismatch at line {} for {:?}", i + 1, path);
    }
}
