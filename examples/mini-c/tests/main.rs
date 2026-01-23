use rusty_c::{
    ast::{BasicType, Declaration, Type},
    config::ReadConfig,
    lexer::CLexer,
    parser::CParser,
    MiniCFrontend,
};
use std::{fs::read_to_string, io::Write};
use tempfile::NamedTempFile;

#[test]
fn test_parse_simple_c_file() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "int main() {{").unwrap();
    writeln!(temp_file, "    return 0;").unwrap();
    writeln!(temp_file, "}}").unwrap();

    let mut frontend = MiniCFrontend::new();
    let source = read_to_string(temp_file.path()).unwrap();
    let result = frontend.parse(&source);

    assert!(result.is_ok());
}

#[test]
fn test_compile_to_gaia() {
    let source = "int main() { return 0; }";

    let mut frontend = MiniCFrontend::new();
    let result = frontend.compile_to_gaia(source);
    assert!(result.is_ok());

    let gaia_program = result.unwrap();
    assert!(!gaia_program.functions.is_empty());
}

#[test]
fn test_tokenize_simple_code() {
    let source = "int x = 42;";
    let config = ReadConfig::new();
    let lexer = CLexer::new(&config);

    let diagnostics = lexer.tokenize(source);
    assert!(diagnostics.result.is_ok());

    let token_stream = diagnostics.result.unwrap();
    assert!(!token_stream.tokens.into_inner().is_empty());
}

#[test]
fn test_parse_simple_function() {
    let source = "int main() { return 0; }";
    let config = ReadConfig::new();
    let lexer = CLexer::new(&config);
    let token_stream = lexer.tokenize(source).result.unwrap();

    let mut parser = CParser::new(token_stream);
    let program = parser.parse().unwrap();

    assert_eq!(program.declarations.len(), 1);
    match &program.declarations[0] {
        Declaration::Function { name, return_type, body, .. } => {
            assert_eq!(name, "main");
            assert_eq!(*return_type, Type::Basic(BasicType::Int));
            assert!(body.is_some());
        }
        _ => panic!("Expected function declaration"),
    }
}

#[test]
fn test_parse_function_with_parameters() {
    let source = "int add(int a, int b) { return a + b; }";
    let config = ReadConfig::new();
    let lexer = CLexer::new(&config);
    let token_stream = lexer.tokenize(source).result.unwrap();
    let mut parser = CParser::new(token_stream);
    let program = parser.parse().unwrap();

    assert_eq!(program.declarations.len(), 1);
    match &program.declarations[0] {
        Declaration::Function { name, parameters, .. } => {
            assert_eq!(name, "add");
            assert_eq!(parameters.len(), 2);
            assert_eq!(parameters[0].name.as_deref(), Some("a"));
            assert_eq!(parameters[1].name.as_deref(), Some("b"));
        }
        _ => panic!("Expected function declaration"),
    }
}

#[test]
fn test_parse_variable_declaration() {
    let source = "int x = 42;";
    let config = ReadConfig::new();
    let lexer = CLexer::new(&config);
    let token_stream = lexer.tokenize(source).result.unwrap();
    let mut parser = CParser::new(token_stream);
    let program = parser.parse().unwrap();

    assert_eq!(program.declarations.len(), 1);
    match &program.declarations[0] {
        Declaration::Variable { name, type_, initializer } => {
            assert_eq!(name, "x");
            assert_eq!(*type_, Type::Basic(BasicType::Int));
            assert!(initializer.is_some());
        }
        _ => panic!("Expected variable declaration"),
    }
}
