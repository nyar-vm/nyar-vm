use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::bytecode::format::{Chunk, NyarcModule};
use nyar_vm::vm::interpreter::NyarVM;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};

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

fn run_cmd(path: &str) -> i32 {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return 2,
    };
    let module = match NyarcModule::parse(&data) {
        Ok(m) => m,
        Err(_) => return 3,
    };
    let chunk = match module.chunks.get(0) {
        Some(c) => c.clone(),
        None => return 4,
    };
    let program = match Decoder::new(&chunk.code).decode_all() {
        Ok(p) => p,
        Err(_) => return 6,
    };
    let mut vm = NyarVM::new(module.constants, module.effects);
    match vm.execute(&program) {
        Ok(v) => {
            let _ = io::stdout().write_all(format!("{:?}\n", v.tag).as_bytes());
            0
        }
        Err(_) => 5,
    }
}

fn dump_cmd(path: &str) -> i32 {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return 2,
    };
    let module = match NyarcModule::parse(&data) {
        Ok(m) => m,
        Err(_) => return 3,
    };
    let mut out = String::new();
    out.push_str(&format!("consts:{}\n", module.constants.len()));
    for (i, c) in module.constants.iter().enumerate() {
        out.push_str(&format!("{}:{:?}\n", i, c));
    }
    for (i, ch) in module.chunks.iter().enumerate() {
        out.push_str(&format!("chunk{}:{} bytes\n", i, ch.code.len()));
    }
    let _ = io::stdout().write_all(out.as_bytes());
    0
}

fn repl_cmd() -> i32 {
    let mut buf = String::new();
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    loop {
        buf.clear();
        let _ = io::stdout().write_all(b"> ");
        if handle.read_line(&mut buf).is_err() {
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
                handlers: vec![],
            };
            let program = match Decoder::new(&chunk.code).decode_all() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let mut vm = NyarVM::new(vec![], vec![]);
            let _ = vm.execute(&program);
        }
    }
    0
}

fn bench_cmd() -> i32 {
    let mut code = Vec::new();
    code.push(nyar_vm::bytecode::opcode::Opcode::Push as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(nyar_vm::bytecode::opcode::Opcode::Return as u8);
    let chunk = Chunk {
        locals: 0,
        upvalues: 0,
        max_stack: 8,
        code,
        handlers: vec![],
    };
    let program = Decoder::new(&chunk.code).decode_all().unwrap_or_default();
    let consts = vec![nyar_vm::bytecode::format::Constant::Int(1)];
    let mut vm = NyarVM::new(consts, vec![]);
    let mut acc = 0i64;
    for _ in 0..10000 {
        let _ = vm.execute(&program);
        acc += 1;
    }
    let _ = io::stdout().write_all(format!("{}\n", acc).as_bytes());
    0
}

fn main() {
    let mut args = env::args().collect::<Vec<_>>();
    let code = if args.len() < 2 {
        1
    } else {
        match args[1].as_str() {
            "run" => {
                if args.len() < 3 {
                    1
                } else {
                    run_cmd(&args[2])
                }
            }
            "dump" => {
                if args.len() < 3 {
                    1
                } else {
                    dump_cmd(&args[2])
                }
            }
            "repl" => repl_cmd(),
            "bench" => bench_cmd(),
            _ => 1,
        }
    };
    std::process::exit(code);
}
