use nyar_error::CliError;
use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::bytecode::format::NyarModule;
use nyar_vm::vm::interpreter::NyarVM;
use std::fs;
use std::io::Write;

pub fn run(path: &str) -> Result<(), CliError> {
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
