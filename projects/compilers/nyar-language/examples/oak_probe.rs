use oak_core::{ParseSession, Parser, SourceText};
use oak_valkyrie::{ValkyrieLanguage, ValkyrieParser};

fn main() {
    let src = "[main] micro entry() -> i32 { return 23 }";
    let language = ValkyrieLanguage::default();
    let parser = ValkyrieParser::new(&language);
    let text = SourceText::new(src);
    let mut session = ParseSession::<ValkyrieLanguage>::default();
    let output = parser.parse(&text, &[], &mut session);
    println!("parse ok: {}", output.diagnostics.is_empty());
    for diagnostic in output.diagnostics {
        println!("parse diag: {diagnostic}");
    }
}
