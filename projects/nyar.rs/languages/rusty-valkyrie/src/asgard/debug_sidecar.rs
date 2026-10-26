//! Debug 侧车：`build.mode: dev` 或 `sourcemap` 时额外写出 RenderIR。
use crate::asgard::error::{AsgardError, AsgardResult};
use std::{fs, path::Path};

use crate::asgard::{awsl::LoweredComponent, codegen::encode_mobile_ui_package, config::VoaConfig};

/// 是否应写出 `debug/render-ir.bin`。
pub fn should_emit_render_ir_sidecar(config: &VoaConfig) -> bool {
    config.build.mode == "dev" || config.build.sourcemap
}

/// 写出 `debug/render-ir.bin`（非 Release 交付物）。
pub fn write_render_ir_sidecar(output_dir: &Path, components: &[LoweredComponent]) -> AsgardResult<()> {
    let debug_dir = output_dir.join("debug");
    fs::create_dir_all(&debug_dir).map_err(|e| AsgardError::msg(format!("{e}: {}", "创建 debug 目录失败")))?;
    let bytes = encode_mobile_ui_package(components);
    fs::write(debug_dir.join("render-ir.bin"), bytes).map_err(|e| AsgardError::msg(format!("{e}: {}", "写入 render-ir.bin 失败")))?;
    Ok(())
}
