use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::vm::interpreter::NyarVM;
use std::env;
use std::fs;
use valkyrie_language::compile_text_to_module;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file.vk>", args[0]);
        return;
    }

    let path = &args[1];
    let src = fs::read_to_string(path).expect("Failed to read file");

    match compile_text_to_module(&src) {
        Ok(module) => {
            // Setup VM
            let mut vm = NyarVM::new(
                module.constants,
                module.chunks.clone(),
                module.classes,
                module.traits,
                module.impls,
                module.effects,
            );

            // Capture stdout
            vm.stdout = Some(Box::new(|msg: &str| {
                println!("{}", msg);
            }));

            // Find main chunk (index 0)
            if module.chunks.is_empty() {
                eprintln!("No chunks generated");
                return;
            }

            let main_chunk = &module.chunks[0];
            let decoder = Decoder::new(&main_chunk.code);
            match decoder.decode_all() {
                Ok(instrs) => {
                    if let Err(e) = vm.execute(&instrs) {
                        eprintln!("Runtime Error: {:?}", e);
                    }
                }
                Err(e) => eprintln!("Decode Error: {:?}", e),
            }
        }
        Err(e) => eprintln!("Compile Error: {:?}", e),
    }
}
