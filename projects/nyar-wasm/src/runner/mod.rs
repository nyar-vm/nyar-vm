use std::path::Path;
use wasmtime::{Engine, Linker, Store, Val};
use wasmtime_wasi::WasiCtxBuilder;

pub fn run<P: AsRef<Path>>(path: P) -> wasmtime::Result<()> {
    let engine = Engine::default();

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;
    // wasmtime_wasi::preview2::bindings::wasi::random::insecure::add_to_linker(&mut linker, |s| s)?;

    let wasi = WasiCtxBuilder::new().inherit_stdio().inherit_args()?.inherit_env()?.build();
    let mut store = Store::new(&engine, wasi);

    let input = wasmtime::Module::from_file(&engine, path)?;
    let instance = linker.instantiate(&mut store, &input)?;

    let hello = instance.get_typed_func::<(i32, i32), u32>(&mut store, "add_ab").expect("nothing");
    let output = hello.call(&mut store, (i32::MAX, i32::MAX))?;
    println!("{:#?}", output);

    let hello = instance.get_func(&mut store, "add_ba").expect("nothing");
    let input = vec![Val::F32(10)];
    let mut output = vec![Val::I32(0)];
    hello.call(&mut store, &input, &mut output)?;
    println!("{:#?}", output);
    Ok(())
}
