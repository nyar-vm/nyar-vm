use nyar_types::{CliError, FormatError};
use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::bytecode::format::NyarModule;
use oak_core::source::Source;
use oak_vfs::{DiskVfs, Vfs};
use std::io::Write;

pub fn dump(path: &str) -> Result<(), CliError> {
    let vfs = DiskVfs::new();
    let source = vfs.get_source(path)
        .ok_or_else(|| CliError::Format(FormatError::Text(format!("File not found: {}", path))))?;

    let module = if path.ends_with(".nyar") {
        let text = source.get_text_from(0);
        NyarModule::parse_toml_str(&text)?
    } else {
        let text = source.get_text_from(0);
        NyarModule::parse(text.as_bytes())?
    };
    let target_chunk = std::env::var("NYAR_DUMP_CHUNK")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let mut out = String::new();
    if target_chunk.is_none() {
        out.push_str(&format!("consts:{}\n", module.constants.len()));
        for (i, c) in module.constants.iter().enumerate() {
            out.push_str(&format!("{}:{:?}\n", i, c));
        }
    }
    let instr_limit = std::env::var("NYAR_DUMP_LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(256);
    for (i, ch) in module.chunks.iter().enumerate() {
        if let Some(t) = target_chunk {
            if i != t {
                continue;
            }
        }
        println!("DEBUG: chunk {} len: {}", i, ch.code.len());
        out.push_str(&format!("chunk{}:{} bytes\n", i, ch.code.len()));
        let decoder = Decoder::new(&ch.code);
        if let Ok(instrs) = decoder.decode_all() {
            for (j, ins) in instrs.iter().enumerate() {
                if j >= instr_limit {
                    break;
                }
                out.push_str(&format!("  {:04}: {:?}\n", j, ins));
            }
        }
    }
    std::io::stdout().write_all(out.as_bytes())?;
    Ok(())
}
