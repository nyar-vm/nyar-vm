use nyar_types::{CliError, FormatError};
use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::bytecode::format::NyarModule;
use nyar_vm::vm::interpreter::NyarVM;
use oak_vfs::{DiskVfs, Vfs};
use std::io::Write;

pub fn run(path: &str) -> Result<(), CliError> {
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
    let chunk = module.chunks.get(0).cloned().ok_or(CliError::NoChunk)?;
    let program = Decoder::new(&chunk.code).decode_all()?;
    let mut vm = NyarVM::new(
        module.constants,
        module.chunks,
        module.classes,
        module.traits,
        module.impls,
        module.effects,
    );
    let out = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    {
        let out_clone = out.clone();
        vm.stdout = Some(Box::new(move |msg: &str| {
            if let Ok(mut buf) = out_clone.lock() {
                buf.push(msg.to_string());
            }
            println!("{}", msg);
        }));
    }
    let v = vm.execute(&program)?;
    if let Ok(buf) = out.lock() {
        for line in buf.iter() {
            eprintln!("{}", line);
        }
    }
    std::io::stdout().write_all(format!("{:?}\n", v.tag).as_bytes())?;
    Ok(())
}
