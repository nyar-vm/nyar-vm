use nyar_types::{EGraph, NyarContext, NyarFrontend};
use oak_vfs::MemoryVfs;
use rusty_r::RustyRFrontend;

#[test]
fn test_basic_r_translation() {
    let frontend = RustyRFrontend::new();
    let source = r#"
        x <- 10
        y <- 20
        print(x + y)
    "#;

    let ast = frontend.parse(source).expect("Failed to parse R code");
    assert!(!ast.statements.is_empty());

    let vfs = MemoryVfs::new();
    let mut egraph = EGraph::new();
    let mut ctx = NyarContext::new(&mut egraph, &vfs, 1);

    let id = frontend.lower_unified(&ast, &mut ctx);
    assert!(id != 0usize);
}
