use nyar_error::{CliError, FormatError};
use nyar_vm::bytecode::format::NyarModule;
use std::fs;

pub fn compile(target: &str, input: &str, output: Option<String>) -> Result<(), CliError> {
    let module = if input.ends_with(".nyar") {
        let text = fs::read_to_string(input)?;
        NyarModule::parse_toml_str(&text)?
    } else {
        let data = fs::read(input)?;
        NyarModule::parse(&data)?
    };

    match target {
        _ => Err(CliError::Format(FormatError::Text(format!(
            "Target '{}' is moved to ProjectChomsky",
            target
        )))),
    }
}

fn derive_base(path: &str) -> String {
    let p = std::path::Path::new(path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let dir = p.parent().and_then(|d| d.to_str()).unwrap_or(".");
    format!("{}/{}", dir, stem)
}
