use clap::{Parser, Subcommand};
use nyar_vm::bytecode::decoder::{DecodeError, Decoder};
use nyar_vm::bytecode::format::{Chunk, FormatError, NyarModule};
use nyar_vm::vm::interpreter::NyarVM;
use nyar_vm::vm::VmError;
use valkyrie_minimal::compile_text_to_module;
use std::fs;
use std::io::{BufRead, Write};

#[derive(Debug)]
pub enum CliError {
    Io(std::io::Error),
    Format(FormatError),
    Decode(DecodeError),
    Vm(VmError),
    NoChunk,
}

mod cmds;

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Io(e) => write!(f, "io error: {}", e),
            CliError::Format(FormatError::InvalidHeader) => {
                write!(f, "format error: invalid header")
            }
            CliError::Format(FormatError::Truncated) => write!(f, "format error: truncated data"),
            CliError::Format(FormatError::Text(msg)) => write!(f, "format error: {}", msg),
            CliError::Decode(DecodeError::InvalidOpcode(op)) => {
                write!(f, "decode error: invalid opcode 0x{:02X}", op)
            }
            CliError::Decode(DecodeError::Truncated) => write!(f, "decode error: truncated code"),
            CliError::Vm(VmError::InvalidOpcode) => write!(f, "vm error: invalid opcode"),
            CliError::Vm(VmError::StackUnderflow) => write!(f, "vm error: stack underflow"),
            CliError::Vm(VmError::IndexOutOfBounds) => write!(f, "vm error: index out of bounds"),
            CliError::Vm(VmError::UnhandledEffect(name)) => {
                write!(f, "vm error: unhandled effect {}", name)
            }
            CliError::Vm(VmError::UnhandledError) => write!(f, "vm error: unhandled error"),
            CliError::NoChunk => write!(f, "no chunk to execute"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Io(e)
    }
}
impl From<FormatError> for CliError {
    fn from(e: FormatError) -> Self {
        CliError::Format(e)
    }
}
impl From<DecodeError> for CliError {
    fn from(e: DecodeError) -> Self {
        CliError::Decode(e)
    }
}
impl From<VmError> for CliError {
    fn from(e: VmError) -> Self {
        CliError::Vm(e)
    }
}

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

fn run_cmd(path: &str) -> Result<(), CliError> {
    let module = if path.ends_with(".nyar") {
        let text = fs::read_to_string(path)?;
        NyarModule::parse_toml_str(&text)?
    } else {
        let data = fs::read(path)?;
        NyarModule::parse(&data)?
    };
    let chunk = module.chunks.get(0).cloned().ok_or(CliError::NoChunk)?;
    let program = Decoder::new(&chunk.code).decode_all()?;
    let mut vm = NyarVM::new(module.constants, module.effects);
    let v = vm.execute(&program)?;
    std::io::stdout().write_all(format!("{:?}\n", v.tag).as_bytes())?;
    Ok(())
}

fn dump_cmd(path: &str) -> Result<(), CliError> {
    let module = if path.ends_with(".nyar") {
        let text = fs::read_to_string(path)?;
        NyarModule::parse_toml_str(&text)?
    } else {
        let data = fs::read(path)?;
        NyarModule::parse(&data)?
    };
    let mut out = String::new();
    out.push_str(&format!("consts:{}\n", module.constants.len()));
    for (i, c) in module.constants.iter().enumerate() {
        out.push_str(&format!("{}:{:?}\n", i, c));
    }
    for (i, ch) in module.chunks.iter().enumerate() {
        out.push_str(&format!("chunk{}:{} bytes\n", i, ch.code.len()));
    }
    std::io::stdout().write_all(out.as_bytes())?;
    Ok(())
}

fn repl_cmd() -> Result<(), CliError> {
    let mut buf = String::new();
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    loop {
        buf.clear();
        std::io::stdout().write_all(b"> ")?;
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
                handlers: vec![],
            };
            if let Ok(program) = Decoder::new(&chunk.code).decode_all() {
                let mut vm = NyarVM::new(vec![], vec![]);
                let _ = vm.execute(&program);
            }
        }
    }
    Ok(())
}

fn bench_cmd() -> Result<(), CliError> {
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
    std::io::stdout().write_all(format!("{}\n", acc).as_bytes())?;
    Ok(())
}

fn run_valkyrie_cmd(path: &str) -> Result<(), CliError> {
    let src = fs::read_to_string(path)?;
    let module = compile_text_to_module(&src).map_err(|e| CliError::Format(FormatError::Text(e.to_string())))?;
    let chunk = module.chunks.get(0).cloned().ok_or(CliError::NoChunk)?;
    let program = Decoder::new(&chunk.code).decode_all()?;
    let mut vm = NyarVM::new(module.constants, module.effects);
    let v = vm.execute(&program)?;
    std::io::stdout().write_all(format!("{:?}\n", v.tag).as_bytes())?;
    Ok(())
}

#[derive(Parser)]
#[command(name = "nyar-vm", version, about = "NYAR VM CLI")]
pub struct NyarCli {
    #[command(subcommand)]
    cmds: NyarCommand,
}

#[derive(Subcommand)]
pub enum NyarCommand {
    Run { file: String },
    Dump { file: String },
    Repl,
    Bench,
    Valkyrie { file: String },
}

impl NyarCli {
    pub fn run(&self) -> Result<(), CliError> {
        match &self.cmds {
            NyarCommand::Run { file } => run_cmd(&file),
            NyarCommand::Dump { file } => dump_cmd(&file),
            NyarCommand::Repl => repl_cmd(),
            NyarCommand::Bench => bench_cmd(),
            NyarCommand::Valkyrie { file } => run_valkyrie_cmd(&file),
        }
    }
}
