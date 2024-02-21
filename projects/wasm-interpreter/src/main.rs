use std::path::Path;
use wasmtime::{Config, Engine, Store};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime_wasi::preview2::{WasiCtx, WasiCtxBuilder, WasiView};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config::new();
    config.async_support(true);
    config.wasm_reference_types(true);
    config.wasm_function_references(true);
    config.wasm_component_model(true);

    let engine = Engine::new(&config)?;

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/component.wat");
    let bytes = std::fs::read(&path)?;
    let component = Component::new(&engine, &bytes)?;

    let mut linker = Linker::<ContextView>::new(&engine);
    linker.allow_shadowing(true);
    wasmtime_wasi::preview2::command::add_to_linker(&mut linker)?;

    let mut builder = WasiCtxBuilder::new();
    builder.inherit_stderr();

    let mut store = Store::new(&engine, ContextView::new(ResourceTable::default(), builder.build()));

    let instance = linker.instantiate_async(&mut store, &component).await?;
    let mut exports = instance.exports(&mut store);
    let mut root = exports.root();


    // let f_one = instance.get_typed_func::<(), (i64, )>(&mut store, "tuple1")?;
    // let result = f_one.call_async(&mut store, ()).await?;
    // println!("{:?}", result);

    Ok(())
}

pub struct ContextView {
    wasi: WasiCtx,
    resources: ResourceTable,
}

impl ContextView {
    fn new(table: ResourceTable, wasi: WasiCtx) -> Self {
        Self { resources: table, wasi }
    }
}

impl WasiView for ContextView {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.resources
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}