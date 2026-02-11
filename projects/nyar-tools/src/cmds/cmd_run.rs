use nyar_types::{CliError, FormatError};
use nyar_vm::bytecode::format::NyarModule;
use nyar_vm::vm::core::NyarVM;
use oak_core::source::Source;
use oak_vfs::{DiskVfs, Vfs};
use std::io::Write;
use std::sync::Arc;

pub fn run(path: &str) -> Result<(), CliError> {
    let vfs = DiskVfs::new(std::env::current_dir().unwrap());
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
    
    // Set up platform
    vm.platform = Arc::new(nyar_runtime::runtime::platform::NativePlatform);
    
    // Set up runtime (FFI)
    let registry = nyar_runtime::ffi::FFIRegistry::new();
    registry.register_std();
    vm.runtime = Arc::new(registry);

    let module_idx = vm.load_named_module(module, path.to_string());
    
    let v = vm.execute(module_idx, 0)?;
    
    if !v.is_null() {
        std::io::stdout().write_all(format!("{:?}\n", v).as_bytes())?;
    }
    Ok(())
}
