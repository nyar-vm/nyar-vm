use crate::ast::{Expr, Stmt};
use crate::lexer::{Error, Token};

pub fn parse(tokens: &[Token]) -> Result<Vec<Stmt>, Error> {
    let mut i = 0;
    let mut out = Vec::new();
    while let Some(tok) = tokens.get(i) {
        match tok {
            Token::Eof => break,
            _ => {
                let stmt = parse_stmt(tokens, &mut i)?;
                out.push(stmt);
            }
        }
    }
    Ok(out)
}

fn expect_ident(tokens: &[Token], i: &mut usize) -> Result<String, Error> {
    match tokens.get(*i) {
        Some(Token::Ident(s)) => {
            *i += 1;
            Ok(s.clone())
        }
        _ => Err(Error::Parse("expect identifier".into())),
    }
}

fn parse_stmt(tokens: &[Token], i: &mut usize) -> Result<Stmt, Error> {
    match tokens.get(*i) {
        Some(Token::Let) => {
            *i += 1;
            let name = expect_ident(tokens, i)?;
            match tokens.get(*i) {
                Some(Token::Eq) => { *i += 1; }
                _ => return Err(Error::Parse("expect = after identifier".into())),
            }
            let val = parse_expr(tokens, i)?;
            Ok(Stmt::Let(name, val))
        }
        Some(Token::Micro) => {
            *i += 1;
            let name = expect_ident(tokens, i)?;
            let args = parse_args_decl(tokens, i)?;
            let body = parse_block(tokens, i)?;
            Ok(Stmt::FuncDef(name, args, body))
        }
        Some(Token::Class) => {
            *i += 1;
            let name = expect_ident(tokens, i)?;
            match tokens.get(*i) { Some(Token::LBrace) => { *i += 1; } _ => return Err(Error::Parse("expect {".into())) }
            let mut fields = Vec::new();
            loop {
                match tokens.get(*i) {
                    Some(Token::RBrace) => { *i += 1; break; }
                    Some(Token::Ident(s)) => {
                        fields.push(s.clone());
                        *i += 1;
                        match tokens.get(*i) {
                            Some(Token::Comma) => { *i += 1; continue; }
                            Some(Token::RBrace) => { *i += 1; break; }
                            _ => return Err(Error::Parse("expect , or }".into())),
                        }
                    }
                    _ => return Err(Error::Parse("expect field identifier or }".into())),
                }
            }
            Ok(Stmt::ClassDef(name, fields))
        }
        Some(Token::Trait) => {
            *i += 1;
            let name = expect_ident(tokens, i)?;
            match tokens.get(*i) { Some(Token::LBrace) => { *i += 1; } _ => return Err(Error::Parse("expect {".into())) }
            let mut methods = Vec::new();
            loop {
                match tokens.get(*i) {
                    Some(Token::RBrace) => { *i += 1; break; }
                    Some(Token::Ident(s)) => {
                        methods.push(s.clone());
                        *i += 1;
                        match tokens.get(*i) {
                            Some(Token::Comma) => { *i += 1; continue; }
                            Some(Token::RBrace) => { *i += 1; break; }
                            _ => return Err(Error::Parse("expect , or }".into())),
                        }
                    }
                    _ => return Err(Error::Parse("expect method identifier or }".into())),
                }
            }
            Ok(Stmt::TraitDef(name, methods))
        }
        Some(Token::Impl) => {
            *i += 1;
            let trait_name = expect_ident(tokens, i)?;
            match tokens.get(*i) { Some(Token::For) => { *i += 1; } _ => return Err(Error::Parse("expect for".into())) }
            let class_name = expect_ident(tokens, i)?;
            match tokens.get(*i) { Some(Token::LBrace) => { *i += 1; } _ => return Err(Error::Parse("expect {".into())) }
            let mut methods = Vec::new();
            loop {
                match tokens.get(*i) {
                    Some(Token::RBrace) => { *i += 1; break; }
                    Some(Token::Micro) => {
                        let stmt = parse_stmt(tokens, i)?;
                        methods.push(stmt);
                    }
                    _ => return Err(Error::Parse("expect micro method definition or }".into())),
                }
            }
            Ok(Stmt::ImplDef(trait_name, class_name, methods))
        }
        Some(Token::Return) => {
            *i += 1;
            let val = parse_expr(tokens, i)?;
            Ok(Stmt::Return(val))
        }
        _ => {
            let e = parse_expr(tokens, i)?;
            Ok(Stmt::Expr(e))
        }
    }
}

fn parse_args_decl(tokens: &[Token], i: &mut usize) -> Result<Vec<String>, Error> {
    match tokens.get(*i) { Some(Token::LParen) => { *i += 1; } _ => return Err(Error::Parse("expect (".into())) }
    let mut args = Vec::new();
    loop {
        match tokens.get(*i) {
            Some(Token::RParen) => { *i += 1; break; }
            Some(Token::Ident(s)) => {
                args.push(s.clone());
                *i += 1;
                match tokens.get(*i) {
                    Some(Token::Comma) => { *i += 1; continue; }
                    Some(Token::RParen) => { *i += 1; break; }
                    _ => return Err(Error::Parse("expect , or )".into())),
                }
            }
            _ => return Err(Error::Parse("expect identifier or )".into())),
        }
    }
    Ok(args)
}

fn parse_block(tokens: &[Token], i: &mut usize) -> Result<Vec<Stmt>, Error> {
    match tokens.get(*i) { Some(Token::LBrace) => { *i += 1; } _ => return Err(Error::Parse("expect {".into())) }
    let mut stmts = Vec::new();
    loop {
        match tokens.get(*i) {
            Some(Token::RBrace) => { *i += 1; break; }
            Some(Token::Eof) | None => return Err(Error::Parse("unexpected eof in block".into())),
            _ => {
                stmts.push(parse_stmt(tokens, i)?);
            }
        }
    }
    Ok(stmts)
}

fn parse_primary(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
    match tokens.get(*i) {
        Some(Token::Int(v)) => { *i += 1; Ok(Expr::Int(*v)) }
        Some(Token::Ident(name)) => {
            *i += 1;
            Ok(Expr::Variable(name.clone()))
        }
        Some(Token::New) => {
            *i += 1;
            let name = expect_ident(tokens, i)?;
            // Check for ()
            if let Some(Token::LParen) = tokens.get(*i) {
                *i += 1;
                match tokens.get(*i) { Some(Token::RParen) => { *i += 1; } _ => return Err(Error::Parse("expect )".into())) }
            }
            Ok(Expr::New(name))
        }
        Some(Token::LParen) => {
            *i += 1;
            let e = parse_expr(tokens, i)?;
            match tokens.get(*i) { Some(Token::RParen) => { *i += 1; Ok(e) } _ => Err(Error::Parse("expect )".into())) }
        }
        Some(Token::Pipe) => {
            *i += 1;
            let mut args = Vec::new();
            loop {
                match tokens.get(*i) {
                    Some(Token::Pipe) => { *i += 1; break; }
                    Some(Token::Ident(s)) => {
                        args.push(s.clone());
                        *i += 1;
                        match tokens.get(*i) {
                            Some(Token::Comma) => { *i += 1; continue; }
                            Some(Token::Pipe) => { *i += 1; break; }
                            _ => return Err(Error::Parse("expect , or |".into())),
                        }
                    }
                    _ => return Err(Error::Parse("expect identifier or |".into())),
                }
            }
            let body = parse_block(tokens, i)?;
            Ok(Expr::Closure(args, body))
        }
        _ => Err(Error::Parse("unexpected token".into())),
    }
}

fn parse_postfix(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
    let mut left = parse_primary(tokens, i)?;
    loop {
        match tokens.get(*i) {
            Some(Token::Dot) => {
                *i += 1;
                let field = expect_ident(tokens, i)?;
                left = Expr::GetField(Box::new(left), field);
            }
            Some(Token::LParen) => {
                *i += 1;
                let mut args = Vec::new();
                loop {
                    match tokens.get(*i) {
                        Some(Token::RParen) => { *i += 1; break; }
                        _ => {
                            let e = parse_expr(tokens, i)?;
                            args.push(e);
                            match tokens.get(*i) {
                                Some(Token::Comma) => { *i += 1; continue; }
                                Some(Token::RParen) => { *i += 1; break; }
                                _ => return Err(Error::Parse("expect , or )".into())),
                            }
                        }
                    }
                }
                // Convert to Call
                left = Expr::Call(Box::new(left), args);
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_additive(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
    let mut left = parse_postfix(tokens, i)?;
    loop {
        match tokens.get(*i) {
            Some(Token::Plus) => {
                *i += 1;
                let right = parse_postfix(tokens, i)?;
                left = Expr::Add(Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_relational(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
    let mut left = parse_additive(tokens, i)?;
    loop {
        match tokens.get(*i) {
            Some(Token::Is) => {
                *i += 1;
                let name = expect_ident(tokens, i)?;
                left = Expr::InstanceOf(Box::new(left), name);
            }
            Some(Token::As) => {
                *i += 1;
                let name = expect_ident(tokens, i)?;
                left = Expr::Cast(Box::new(left), name);
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_expr(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
    let left = parse_relational(tokens, i)?;
    match tokens.get(*i) {
        Some(Token::Eq) => {
            *i += 1;
            let right = parse_expr(tokens, i)?;
            match left {
                Expr::GetField(obj, field) => {
                    Ok(Expr::SetField(obj, field, Box::new(right)))
                }
                _ => Err(Error::Parse("Invalid assignment target".into())),
            }
        }
        _ => Ok(left)
    }
}
