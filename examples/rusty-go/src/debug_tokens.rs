
use oak_go::{GoLanguage, GoLexer};
use oak_core::{SourceText, Lexer, ParseSession, Source};

fn main() {
    let source = std::fs::read_to_string("tests/basic.go").unwrap();
    let source_text = SourceText::new(source);
    let language = GoLanguage::default();
    let lexer = GoLexer::new(&language);
    let mut session = ParseSession::<GoLanguage>::default();
    let output = lexer.lex(&source_text, &[], &mut session);
    
    if let Ok(tokens) = output.result.as_ref() {
        for token in tokens.iter() {
            let text = source_text.get_text_in(token.span.clone());
            println!("{:?}: {:?} ({:?})", token.kind, text, token.span);
        }
    } else {
        println!("Lexer errors: {:?}", output.diagnostics);
    }
}
