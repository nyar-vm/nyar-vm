use std::path::Path;

use wasmtime_wasi::preview2::WasiView;

mod host;
mod vm;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/component.wat");
    crate::vm::build_vm(&path).await?;
    Ok(())
}
