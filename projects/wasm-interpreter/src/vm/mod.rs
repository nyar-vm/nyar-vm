use wasmtime::{
    component::{Component, Instance, Linker, ResourceTable},
    Config, Engine, Store,
};
use wasmtime_wasi::preview2::{WasiCtx, WasiCtxBuilder, WasiView};

use crate::{Debugger, host::NyarExtension};

pub struct NyarVM {
    store: Store<ContextView>,
    instance: Instance,
}

impl NyarVM {
    pub async fn load_wast(wast: &str) -> anyhow::Result<Self> {
        let engine = get_engine()?;

        let component = Component::new(&engine, wast.as_bytes())?;

        let mut linker = Linker::<ContextView>::new(&engine);
        linker.allow_shadowing(true);
        wasmtime_wasi::preview2::command::add_to_linker(&mut linker)?;

        Debugger::add_to_linker(&mut linker, |state| &mut state.extension)?;

        let mut builder = WasiCtxBuilder::new();
        builder.inherit_stderr();

        let mut store = Store::new(&engine, ContextView::new(ResourceTable::default(), builder.build()));
        let instance = linker.instantiate_async(&mut store, &component).await?;
        Ok(Self { store, instance })
    }
}

fn get_engine() -> anyhow::Result<Engine> {
    let mut config = Config::new();
    config.async_support(true);
    config.wasm_reference_types(true);
    config.wasm_function_references(true);
    config.wasm_component_model(true);
    Engine::new(&config)
}

pub struct ContextView {
    wasi: WasiCtx,
    resources: ResourceTable,
    extension: NyarExtension,
}

impl ContextView {
    fn new(table: ResourceTable, wasi: WasiCtx) -> Self {
        Self { resources: table, wasi, extension: NyarExtension {} }
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
