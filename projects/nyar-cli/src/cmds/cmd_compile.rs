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
        t if t.eq_ignore_ascii_case("wasm") => compile_wasm(&module, input, output),
        t if t.eq_ignore_ascii_case("jvm") => compile_jvm(&module, input, output),
        _ => Err(CliError::Format(FormatError::Text(format!("unknown target: {}", target)))),
    }
}

fn compile_wasm(module: &NyarModule, input: &str, output: Option<String>) -> Result<(), CliError> {
    let wasm = nyar_wasm::compile_module_to_wasm(module).map_err(CliError::from)?;
    let base = output.unwrap_or_else(|| derive_base(input));
    let wasm_path = format!("{}.wasm", base);
    fs::write(&wasm_path, &wasm)?;

    // JS 绑定已移除，生成纯 Wasm 组件/模块
    Ok(())
}

fn compile_jvm(module: &NyarModule, input: &str, output: Option<String>) -> Result<(), CliError> {
    let class = nyar_jvm::compile_module_to_jvm(module).map_err(CliError::from)?;
    let base = output.unwrap_or_else(|| derive_base(input));
    let jar_path = format!("{}.jar", base);
    nyar_jvm::write_jar(&jar_path, &class).map_err(CliError::from)?;
    Ok(())
}

fn derive_base(path: &str) -> String {
    let p = std::path::Path::new(path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let dir = p.parent().and_then(|d| d.to_str()).unwrap_or(".");
    format!("{}/{}", dir, stem)
}
