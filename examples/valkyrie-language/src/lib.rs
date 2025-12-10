use nyar_vm::bytecode::format::{Constant, NyarcModule};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    LParen,
    RParen,
    Comma,
    Plus,
    Int(i64),
    Eof,
}

#[derive(Debug)]
pub enum Error {
    Lex(String),
    Parse(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Lex(s) => write!(f, "lex error: {}", s),
            Error::Parse(s) => write!(f, "parse error: {}", s),
        }
    }
}

impl std::error::Error for Error {}

pub fn lex(input: &str) -> Result<Vec<Token>, Error> {
    let mut out = Vec::new();
    let mut i = 0;
    let b = input.as_bytes();
    while i < b.len() {
        let c = b[i];
        if c == b' ' || c == b'\n' || c == b'\r' || c == b'\t' { i += 1; continue; }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' { while i < b.len() && b[i] != b'\n' { i += 1; } continue; }
        if c == b'(' { out.push(Token::LParen); i += 1; continue; }
        if c == b')' { out.push(Token::RParen); i += 1; continue; }
        if c == b',' { out.push(Token::Comma); i += 1; continue; }
        if c == b'+' { out.push(Token::Plus); i += 1; continue; }
        if (c as char).is_ascii_digit() || c == b'-' {
            let start = i; i += 1; while i < b.len() && (b[i] as char).is_ascii_digit() { i += 1; }
            let s = std::str::from_utf8(&b[start..i]).map_err(|_| Error::Lex("utf8".to_string()))?;
            let v = s.parse::<i64>().map_err(|_| Error::Lex("int".to_string()))?;
            out.push(Token::Int(v)); continue;
        }
        if (c as char).is_ascii_alphabetic() || c == b'_' {
            let start = i; i += 1; while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') { i += 1; }
            let s = std::str::from_utf8(&b[start..i]).map_err(|_| Error::Lex("utf8".to_string()))?;
            out.push(Token::Ident(s.to_string())); continue;
        }
        return Err(Error::Lex(format!("unexpected byte {}", c)));
    }
    out.push(Token::Eof);
    Ok(out)
}

#[derive(Clone)]
pub enum Expr {
    Int(i64),
    Call(String, Vec<Expr>),
    Add(Box<Expr>, Box<Expr>),
}

#[derive(Clone)]
pub enum Stmt { Expr(Expr) }

pub fn parse(tokens: &[Token]) -> Result<Vec<Stmt>, Error> {
    fn parse_primary(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
        match tokens.get(*i) {
            Some(Token::Int(v)) => { *i += 1; Ok(Expr::Int(*v)) }
            Some(Token::Ident(name)) => {
                *i += 1;
                match tokens.get(*i) { Some(Token::LParen) => { *i += 1; } _ => return Err(Error::Parse("expect (".into())) }
                let mut args = Vec::new();
                loop {
                    match tokens.get(*i) {
                        Some(Token::RParen) => { *i += 1; break; }
                        _ => {
                            let e = parse_expr(tokens, i)?;
                            args.push(e);
                            match tokens.get(*i) { Some(Token::Comma) => { *i += 1; continue; } Some(Token::RParen) => { *i += 1; break; } _ => return Err(Error::Parse("expect , or )".into())) }
                        }
                    }
                }
                Ok(Expr::Call(name.clone(), args))
            }
            Some(Token::LParen) => {
                *i += 1;
                let e = parse_expr(tokens, i)?;
                match tokens.get(*i) { Some(Token::RParen) => { *i += 1; Ok(e) } _ => Err(Error::Parse("expect )".into())) }
            }
            _ => Err(Error::Parse("unexpected token".into())),
        }
    }
    fn parse_term(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
        parse_primary(tokens, i)
    }
    fn parse_expr(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
        let mut left = parse_term(tokens, i)?;
        loop {
            match tokens.get(*i) { Some(Token::Plus) => { *i += 1; let right = parse_term(tokens, i)?; left = Expr::Add(Box::new(left), Box::new(right)); } _ => break }
        }
        Ok(left)
    }
    let mut i = 0;
    let mut out = Vec::new();
    while let Some(tok) = tokens.get(i) {
        match tok { Token::Eof => break, _ => { let e = parse_expr(tokens, &mut i)?; out.push(Stmt::Expr(e)); } }
    }
    Ok(out)
}

pub fn compile(stmts: &[Stmt]) -> NyarcModule {
    fn desc_index(consts: &mut Vec<Constant>, name: &str) -> u16 {
        if let Some((i, _)) = consts.iter().enumerate().find(|(_, c)| matches!(c, Constant::String(s) if s == name)) { i as u16 } else { let i = consts.len() as u16; consts.push(Constant::String(name.to_string())); i }
    }
    fn compile_expr(code: &mut Vec<u8>, consts: &mut Vec<Constant>, e: &Expr) {
        match e {
            Expr::Int(v) => {
                let idx = consts.len() as u16; consts.push(Constant::Int(*v));
                code.push(nyar_vm::bytecode::opcode::Opcode::Push as u8);
                code.extend_from_slice(&idx.to_le_bytes());
            }
            Expr::Add(a, b) => {
                compile_expr(code, consts, a);
                compile_expr(code, consts, b);
                let didx = desc_index(consts, "add");
                code.push(nyar_vm::bytecode::opcode::Opcode::FFICall as u8);
                code.extend_from_slice(&didx.to_le_bytes());
                code.push(2u8);
            }
            Expr::Call(name, args) => {
                for arg in args { compile_expr(code, consts, arg); }
                let didx = desc_index(consts, name);
                code.push(nyar_vm::bytecode::opcode::Opcode::FFICall as u8);
                code.extend_from_slice(&didx.to_le_bytes());
                code.push(args.len() as u8);
            }
        }
    }
    let mut code = Vec::new();
    let mut consts = Vec::new();
    for s in stmts { if let Stmt::Expr(e) = s { compile_expr(&mut code, &mut consts, e); } }
    code.push(nyar_vm::bytecode::opcode::Opcode::Halt as u8);
    nyar_vm::bytecode::format::minimal_module_with_chunk(code, consts)
}

pub fn compile_text_to_module(src: &str) -> Result<NyarcModule, Error> {
    let toks = lex(src)?;
    let ast = parse(&toks)?;
    Ok(compile(&ast))
}
