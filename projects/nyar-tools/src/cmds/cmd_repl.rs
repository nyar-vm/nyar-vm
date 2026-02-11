use nyar_types::CliError;
use nyar_vm::bytecode::format::{Chunk, NyarModule};
use nyar_vm::vm::core::NyarVM;
use std::io::{Write, BufRead};
use std::sync::Arc;

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let t = s.as_bytes();
    if t.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(t.len() / 2);
    for i in (0..t.len()).step_by(2) {
        let h = t[i];
        let l = t[i + 1];
        let hn = match h {
            b'0'..=b'9' => h - b'0',
            b'a'..=b'f' => h - b'a' + 10,
            b'A'..=b'F' => h - b'A' + 10,
            _ => return None,
        };
        let ln = match l {
            b'0'..=b'9' => l - b'0',
            b'a'..=b'f' => l - b'a' + 10,
            b'A'..=b'F' => l - b'A' + 10,
            _ => return None,
        };
        out.push((hn << 4) | ln);
    }
    Some(out)
}

pub fn repl() -> Result<(), CliError> {
    let mut buf = String::new();
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    loop {
        buf.clear();
        std::io::stdout().write_all(b"> ")?;
        std::io::stdout().flush()?;
        let n = handle.read_line(&mut buf)?;
        if n == 0 {
            break;
        }
        let line = buf.trim();
        if line.is_empty() {
            continue;
        }
        if line == "exit" {
            break;
        }
        let bytes = decode_hex(&line.replace(" ", ""));
        if let Some(code) = bytes {
            let chunk = Chunk {
                locals: 0,
                upvalues: 0,
                max_stack: 8,
                code,
                ..Default::default()
            };
            let module = NyarModule {
                chunks: vec![chunk],
                ..Default::default()
            };
            let mut vm = NyarVM::new();
            
            // Set up platform and runtime
            vm.platform = Arc::new(nyar_runtime::runtime::platform::NativePlatform);
            let registry = nyar_runtime::ffi::FFIRegistry::new();
            registry.register_std();
            vm.runtime = Arc::new(registry);
            
            let module_idx = vm.load_module(module);
            let _ = vm.execute(module_idx, 0);
        }
    }
    Ok(())
}
