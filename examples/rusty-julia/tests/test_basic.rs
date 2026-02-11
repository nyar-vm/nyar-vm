use rusty_julia::RustyJuliaFrontend;
use nyar_types::{NyarContext, NyarFrontend, Vfs};
use oak_vfs::MemVfs;
use chomsky_uir::ConstraintAnalysis;

#[test]
fn test_hello() {
    let source = include_str!("hello.jl");
    let frontend = RustyJuliaFrontend::new();
    let ast = frontend.parse(source).expect("Failed to parse");
    
    let mut vfs = MemVfs::default();
    let mut ctx = NyarContext::<MemVfs, ConstraintAnalysis>::new(&mut vfs);
    
    let id = frontend.lower_unified(&ast, &mut ctx);
    assert!(id.index() > 0);
}
