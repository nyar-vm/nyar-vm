use rusty_fortran::RustyFortranFrontend;
use nyar_types::NyarFrontend;
use oak_vfs::MemoryVfs;

#[test]
fn test_parse_hello() {
    let frontend = RustyFortranFrontend::default();
    let source = r#"program hello
  print *, "Hello, Fortran!"
end program hello
"#;
    let ast = frontend.parse(source).expect("Failed to parse Fortran source");
    let vfs = MemoryVfs::new();
    let _tree = frontend.lower(&ast, &vfs).expect("Failed to lower AST to UIR");
}
