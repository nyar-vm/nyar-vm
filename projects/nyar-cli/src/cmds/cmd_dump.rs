use nyar_error::CliError;
use nyar_vm::bytecode::format::NyarModule;
use std::fs;
use std::io::Write;

pub fn dump(path: &str) -> Result<(), CliError> {
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
