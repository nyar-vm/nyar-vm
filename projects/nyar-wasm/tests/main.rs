#[test]
fn ready() {
    println!("it works!")
}

use anyhow::Result;
use nyar_wasm::{StartupConfig, WasmEngine};
use wasmtime::*;

#[test]
fn main() -> Result<()> {
    let mut engine = WasmEngine::new(StartupConfig {});
    engine.evaluate_wat(include_str!("step1.wat")).unwrap();
    engine.evaluate_wat(include_str!("step2.wat")).unwrap();
    Ok(())
}
