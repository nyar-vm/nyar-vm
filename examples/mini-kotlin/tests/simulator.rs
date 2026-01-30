use mini_kotlin::wasm::MyView;
use mini_kotlin::visitor::{Dog, Cat, Fish, sound};
use mini_kotlin::tagless::{expr, SetContext, SoundContext, Neg};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Linker},
};
use wasmtime_wasi::add_to_linker_async;
use wat::{GenerateDwarf, Parser};
use std::marker::PhantomData;

#[tokio::test]
async fn test_wasm_simulator() -> anyhow::Result<()> {
    let mut parser = Parser::new();
    parser.generate_dwarf(GenerateDwarf::Full);
    // Path is relative to the package root
    let wasm_bytes = parser.parse_file("src/wasm/com.wat")?;

    let mut wasm = File::create("src/wasm/com.wasm").await?;
    wasm.write_all(&wasm_bytes).await?;


    // Create a store and instantiate the module
    let mut config = Config::new();
    config
        .async_support(true)
        .wasm_function_references(true)
        .wasm_gc(true)
        .wasm_component_model(true)
        .wasm_component_model_multiple_returns(true);

    let engine = Engine::new(&config)?;
    let module = Component::new(&engine, wasm_bytes)?;
    let mut store = Store::new(&engine, MyView::default());
    let mut linker = Linker::new(&engine);
    add_to_linker_async(&mut linker)?;
    let _instance = linker.instantiate_async(&mut store, &module).await?;

    Ok(())
}

#[test]
fn test_visitor() {
    let dog = Dog { name: "nana".to_string() };
    let cat = Cat { name: "nvnv".to_string() };
    let fish = Fish { name: "fish".to_string() };
    sound(&dog);
    sound(&cat);
    sound(&fish);
}

#[test]
fn test_tagless() {
    assert_eq!(expr::<SetContext>(), 45);
    assert_eq!(expr::<SoundContext>(), "(42) + ((1) + (2))");
    assert_eq!(expr::<Neg<SetContext>>(), -45);
    assert_eq!(expr::<Neg<SoundContext>>(), "(-42) + ((-1) + (-2))");
}
