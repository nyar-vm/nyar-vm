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

#[test]
fn merge() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::default();
    // 创建 Wasmtime 实例
    let mut store = Store::<()>::default();

    // 从嵌入的字符串中加载模块
    let module1 = Module::new(&engine, include_str!("step1.wat"))?;
    let module2 = Module::new(&engine, include_str!("step2.wat"))?;

    let mut instance = Linker::new(&engine)
        // load old
        .module(&mut store, "@context", &module1)?
        .instantiate(&mut store, &module2)?;

    // 调用导出函数
    let combined_func = match instance.get_func(&mut store, "_start") {
        Some(s) => s,
        None => {
            panic!("missing _start")
        }
    };

    // 调用合并后的函数
    let mut exit = vec![Val::I32(0)];
    combined_func.call(&mut store, &[], &mut exit)?;

    println!("Result: {:?}", exit);

    Ok(())
}
