
use oak_go::{GoLanguage, GoLexer, GoSyntaxKind};
use oak_core::{SourceText, Lexer, LexerCache};

fn main() {
    let source = std::fs::read_to_string("examples/rusty-go/tests/basic.go").unwrap();
    let source_text = SourceText::new(source.clone());
    let language = GoLanguage::default();
    let lexer = GoLexer::new(&language);
    let mut cache = oak_core::lexer::SimpleLexerCache::default();
    let output = lexer.lex(&source_text, &[], &mut cache);
    
    for token in output.tokens {
        let text = source_text.get_text_in(token.span.clone());
        println!("{:?}: {:?} ({:?})", token.kind, text, token.span);
    }
}
