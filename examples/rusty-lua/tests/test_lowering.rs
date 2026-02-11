use rusty_lua::RustyLuaFrontend;
use nyar_types::{NyarFrontend, NyarContext, Vfs, Loc};
use nyar_vm::NyarVM;
use chomsky_uir::ConstraintAnalysis;
use std::path::Path;

#[test]
fn test_hello_lua() {
    let frontend = RustyLuaFrontend::new();
    let source = include_str!("hello.lua");
    let ast = frontend.parse(source).expect("Failed to parse Lua source");
    
    let mut vm = NyarVM::new();
    let mut ctx = NyarContext::new(vm.vfs().clone());
    
    let id = frontend.lower_unified(&ast, &mut ctx);
    assert!(id.as_usize() > 0);
    
    println!("Lowered ID: {:?}", id);
}
