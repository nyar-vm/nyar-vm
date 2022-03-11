use wasmtime::{Caller, Engine, Func, Instance, Store, Val};

pub fn run(wasm: &[u8]) -> wasmtime::Result<()> {
    let engine = Engine::default();
    let module = wasmtime::Module::from_binary(&engine, wasm)?;

    // All wasm objects operate within the context of a "store". Each
    // `Store` has a type parameter to store host-specific data, which in
    // this case we're using `4` for.
    let mut store = Store::new(&engine, 4);
    let host_func = Func::wrap(&mut store, |caller: Caller<'_, u32>, param: i32| {
        println!("Got {} from WebAssembly", param);
        println!("my host state is: {}", caller.data());
    });

    // Instantiation of a module requires specifying its imports and then
    // afterwards we can fetch exports by name, as well as asserting the
    // type signature of the function with `get_typed_func`.
    let instance = Instance::new(&mut store, &module, &[])?;
    let hello = instance.get_func(&mut store, "add_ba").expect("nothing");

    // And finally we can call the wasm!
    let input = vec![Val::I32(58)];
    let mut output = vec![Val::I32(0)];
    hello.call(&mut store, &input, &mut output)?;
    println!("{:#?}", input);
    println!("{:#?}", output);
    Ok(())
}