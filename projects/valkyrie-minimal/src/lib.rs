use nyar_vm::bytecode::format::{Constant, NyarcModule};
use nyar_vm::bytecode::format::minimal_module_with_chunk;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Return,
    Int(i64),
    Eof,
}

#[derive(Debug)]
pub enum MiniError {
    Lex(String),
    Parse(String),
}

impl std::fmt::Display for MiniError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MiniError::Lex(s) => write!(f, "lex error: {}", s),
            MiniError::Parse(s) => write!(f, "parse error: {}", s),
        }
    }
}

impl std::error::Error for MiniError {}

pub fn lex(input: &str) -> Result<Vec<Token>, MiniError> {
    let mut out = Vec::new();
    let mut i = 0;
    let b = input.as_bytes();
    while i < b.len() {
        let c = b[i];
        if c == b' ' || c == b'\n' || c == b'\r' || c == b'\t' { i += 1; continue; }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' { while i < b.len() && b[i] != b'\n' { i += 1; } continue; }
        if (c as char).is_ascii_digit() || c == b'-' {
            let start = i;
            i += 1;
            while i < b.len() && (b[i] as char).is_ascii_digit() { i += 1; }
            let s = std::str::from_utf8(&b[start..i]).map_err(|_| MiniError::Lex("utf8".to_string()))?;
            let v = s.parse::<i64>().map_err(|_| MiniError::Lex("int".to_string()))?;
            out.push(Token::Int(v));
            continue;
        }
        if i + 6 <= b.len() && &b[i..i + 6] == b"return" { out.push(Token::Return); i += 6; continue; }
        return Err(MiniError::Lex(format!("unexpected byte {}", c)));
    }
    out.push(Token::Eof);
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt(pub i64);

pub fn parse(tokens: &[Token]) -> Result<ReturnStmt, MiniError> {
    let mut i = 0;
    match tokens.get(i) { Some(Token::Return) => { i += 1; } _ => return Err(MiniError::Parse("expect return".to_string())) }
    let v = match tokens.get(i) { Some(Token::Int(x)) => { i += 1; *x } _ => return Err(MiniError::Parse("expect int".to_string())) };
    match tokens.get(i) { Some(Token::Eof) => {} _ => return Err(MiniError::Parse("trailing tokens".to_string())) }
    Ok(ReturnStmt(v))
}

pub fn compile(ast: ReturnStmt) -> NyarcModule {
    let mut code = Vec::new();
    code.push(nyar_vm::bytecode::opcode::Opcode::Push as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(nyar_vm::bytecode::opcode::Opcode::Return as u8);
    let consts = vec![Constant::Int(ast.0)];
    minimal_module_with_chunk(code, consts)
}

pub fn compile_text_to_module(src: &str) -> Result<NyarcModule, MiniError> {
    let toks = lex(src)?;
    let ast = parse(&toks)?;
    Ok(compile(ast))
}

pub fn to_toml_string(m: &NyarcModule) -> Result<String, MiniError> {
    toml::to_string(m).map_err(|e| MiniError::Parse(e.to_string()))
}

