use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/component.wat");
    let wast = std::fs::read_to_string(&path)?;
    NyarVM::load_wast(&wast).await?;
    Ok(())
}
