use std::path::Path;

use nyar_runtime::NyarVM;

#[tokio::test]
async fn main() -> anyhow::Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/component.wat");
    let wast = std::fs::read_to_string(&path)?;
    NyarVM::load_wast(&wast).await?;
    Ok(())
}
