use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Instance, Linker, ResourceTable},
};
use wasmtime_wasi::{WasiCtx, WasiView, add_to_linker_async, Stdout, AsyncStdoutStream};
use wat::{GenerateDwarf, Parser};

pub struct MyView {
    table: ResourceTable,
    wasi: WasiCtx,
}

impl Default for MyView {
    fn default() -> Self {
        Self { table: Default::default(), wasi: WasiCtx::builder().stdout(wasmtime_wasi::Stdout).stderr(wasmtime_wasi::Stderr).build() }
    }
}

impl WasiView for MyView {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}
