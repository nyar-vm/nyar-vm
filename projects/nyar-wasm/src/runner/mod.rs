use wasmtime::{Caller, Engine, Extern, Func, Instance, Linker, Store, Val};
use wasmtime_wasi::WasiCtxBuilder;

pub fn run(wasm: &[u8]) -> wasmtime::Result<()> {
    let engine = Engine::default();

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

    let wasi = WasiCtxBuilder::new().inherit_stdio().inherit_args()?.inherit_env()?.build();
    let mut store = Store::new(&engine, wasi);

    let input = wasmtime::Module::from_binary(&engine, wasm)?;
    let instance = linker.instantiate(&mut store, &input)?;
    // let instance2 = linker.instance(&mut store, "running", instance)?;

    // All wasm objects operate within the context of a "store". Each
    // `Store` has a type parameter to store host-specific data, which in
    // this case we're using `4` for.
    // let mut store = Store::new(&engine, 4096);
    // let host_func = Func::wrap(&mut store, |caller: Caller<'_, u32>, param: i32| {
    //     println!("Got {} from WebAssembly", param);
    //     println!("my host state is: {}", caller.data());
    // });

    // Instantiation of a module requires specifying its imports and then
    // afterwards we can fetch exports by name, as well as asserting the
    // type signature of the function with `get_typed_func`.
    // let instance = Instance::new(&mut store, &module, &[Extern::Func(host_func)])?;

    // let modules = instance2.instantiate()
    // for i in modules.exports() {
    //     println!("export {:?}", i)
    // }

    let hello = instance.get_typed_func::<(i32, i32), u32>(&mut store, "add_ab").expect("nothing");
    let output = hello.call(&mut store, (0, 0))?;
    println!("{:#?}", output);

    let hello = instance.get_func(&mut store, "add_ba").expect("nothing");
    let input = vec![Val::I32(0)];
    let mut output = vec![Val::I32(0)];
    hello.call(&mut store, &input, &mut output)?;
    println!("{:#?}", output);
    Ok(())
}
