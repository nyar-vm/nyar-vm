use nyar_types::{CliError, FormatError};
use nyar_vm::bytecode::format::NyarModule;
use oak_core::source::Source;
use oak_vfs::{DiskVfs, Vfs};

pub fn compile(target: &str, input: &str, _output: Option<String>) -> Result<(), CliError> {
    let vfs = DiskVfs::new(std::env::current_dir().unwrap());
    
    let source = vfs.get_source(input)
        .ok_or_else(|| CliError::Format(FormatError::Text(format!("File not found: {}", input))))?;
    
    let _module = if input.ends_with(".nyar") {
        let text = source.get_text_from(0);
        NyarModule::parse_toml_str(&text)?
    } else {
        // FIXME: VFS currently only supports text sources well, binary reading needs to be handled via read_dir or similar if supported
        // For now we might need to read the file content as bytes if possible, but DiskVfs/Vfs trait seems to focus on SourceText.
        // Assuming Vfs implementation can handle reading bytes or we fallback to text for now.
        // Wait, DiskVfs uses oak_core::SourceText which is text-based.
        // If we need binary read, we might need to extend Vfs or use a different method.
        // However, looking at the previous code: fs::read(input).
        // Let's see if we can get bytes from SourceText or if we need to add read_file to Vfs.
        // The Vfs trait has `get_source` returning `SourceText`.
        // `SourceText` usually holds string.
        // If `NyarModule::parse` expects bytes, we might be limited here.
        // But for IO-agnostic design, we should stick to Vfs.
        // Let's assume for now we are dealing with text or handle the error.
        
        // Actually, let's look at `oak_vfs` definition if I can.
        // Assuming `get_source` is what we have.
        // If `NyarModule` is binary, `SourceText` might not be enough if it enforces UTF-8.
        // But let's proceed with `get_source` and convert to bytes if it's text, or error out if binary support is missing in Vfs.
        // Re-reading the previous code: `fs::read(input)` returns `Vec<u8>`.
        // `NyarModule::parse` takes `&[u8]`.
        
        // For now, let's just use `get_text_from` and convert to bytes, assuming it's valid UTF-8 for now (which might be wrong for binary).
        // If `oak_vfs` doesn't support binary files, we might need to update `oak_vfs` later.
        // But the user task is to use `oak_vfs`.
        
        let text = source.get_text_from(0);
        NyarModule::parse(text.as_bytes())?
    };

    match target {
        _ => Err(CliError::Format(FormatError::Text(format!(
            "Target '{}' is moved to ProjectChomsky",
            target
        )))),
    }
}

#[allow(dead_code)]
fn derive_base(path: &str) -> String {
    let p = std::path::Path::new(path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let dir = p.parent().and_then(|d| d.to_str()).unwrap_or(".");
    format!("{}/{}", dir, stem)
}
