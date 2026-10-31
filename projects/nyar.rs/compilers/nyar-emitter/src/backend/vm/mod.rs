//! Emit `.nyar` bytecode modules for the Nyar VM backend.

use std::{fs, path::Path};

use crate::{EmitterError, Result};
use odin_bytecode::{OdinModuleData, encode_module};

/// Encode [`OdinModuleData`] and write it to `output_path`.
pub fn emit_nyar_module(module: &OdinModuleData, output_path: &Path) -> Result<()> {
    let bytes = encode_module(module);

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("failed to create output directory: {}: {e}", parent.display()))?;
        }
    }

    fs::write(output_path, &bytes).map_err(|e| format!("failed to write nyar module: {}: {e}", output_path.display()))?;

    Ok(())
}
