use std::str::FromStr;

use wast_encoder::{
    CanonicalWasi, DependentRegistry, ExternalFunction, Identifier, ResolveDependencies, VariantItem, VariantType, WasiModule,
    WasiParameter, WasiResource, WasiType,
};

#[test]
fn test_hello_world() {
    let mut global = DependentRegistry::default();

    let mut source = CanonicalWasi::default();
    let wasi_io_error = WasiModule::from_str("wasi:io/error@0.2.0").unwrap();
    let wasi_io_streams = WasiModule::from_str("wasi:io/streams@0.2.0").unwrap();

    global += WasiResource::new(wasi_io_error.clone(), "error", "std::io::IoError");
    global += WasiResource::new(wasi_io_streams.clone(), "output-stream", "std::io::OutputStream");
    let mut stream_error = VariantType::new("std::io::StreamError");
    stream_error += VariantItem::new("LastOperationFailed")
        .with_fields(WasiType::TypeHandler { name: Identifier::from_str("std::io::IoError").unwrap(), own: true });
    stream_error += VariantItem::new("Closed");
    global += stream_error;

    let mut f1 = ExternalFunction::new(
        wasi_io_streams.clone(),
        "blocking-write-and-flush",
        "std::io::OutputStream::blocking_write_and_flush",
    );
    f1 += WasiParameter::new(
        "self",
        WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: false },
    );
    f1 += WasiType::Result {
        success: None,
        failure: Some(Box::new(WasiType::TypeAlias { name: Identifier::from_str("std::io::StreamError").unwrap() })),
    };
    global += f1;
    for (module, instance) in global.group_instances() {
        let dep = instance.dependent_modules(&global);
        println!("{module}: {dep:?}")
    }

    let wast = source.encode();
    println!("{wast}");
}
