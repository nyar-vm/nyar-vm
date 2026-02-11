use nyar_types::{CliError, FormatError};
use nyar_vm::bytecode::format::NyarModule;
use nyar_vm::vm::NyarVM;
use nyar_vm::NyarDriver;
use oak_core::source::Source;
use oak_vfs::{DiskVfs, Vfs};
use std::io::Write;

pub fn run(path: &str) -> Result<(), CliError> {
    if path.ends_with(".lua") {
        let frontend = rusty_lua::RustyLuaFrontend::new();
        let driver = NyarDriver::new();
        let vfs = driver.default_vfs();
        driver
            .run_source(&frontend, &vfs, path)
            .map_err(|e| CliError::Format(FormatError::Text(format!("Lua execution error: {:?}", e))))?;
        return Ok(());
    }

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
    let mut vm = NyarVM::new();
    vm.platform = std::sync::Arc::new(nyar_vm::runtime::platform::NativePlatform);
    let module_idx = vm.load_named_module(module, path.to_string());
    
    let v = vm.execute(module_idx, 0)?;
    
    std::io::stdout().write_all(format!("{:?}\n", v).as_bytes())?;
    Ok(())
}
