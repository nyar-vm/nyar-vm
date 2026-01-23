use pe_assembler::{
    exports::nyar::pe_assembly::{easy_test::Guest, types::TargetArch},
    PeContext,
};

mod windows;

#[test]
fn ready() {
    println!("it works!")
}

#[test]
fn easy_hello() {
    let _context = PeContext {};
    PeContext::easy_console_log(TargetArch::X64, "hello world".to_string()).unwrap();
}
