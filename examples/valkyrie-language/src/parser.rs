use crate::ast::{Expr, Stmt, Pattern};
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

fn parse_if(tokens: &[Token], i: &mut usize) -> Result<Stmt, Error> {
    match tokens.get(*i) { Some(Token::If) => { *i += 1; } _ => return Err(Error::Parse("expect if".into())) }
    let cond = parse_expr(tokens, i)?;
    let then_body = parse_block(tokens, i)?;
    let else_body = if let Some(Token::Else) = tokens.get(*i) {
        *i += 1;
        if let Some(Token::If) = tokens.get(*i) {
            let nested = parse_if(tokens, i)?;
            Some(vec![nested])
        } else {
            Some(parse_block(tokens, i)?)
        }
    } else { None };
    Ok(Stmt::If(cond, then_body, else_body))
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

fn parse_pattern(tokens: &[Token], i: &mut usize) -> Result<Pattern, Error> {
    match tokens.get(*i) {
        Some(Token::Int(v)) => { *i += 1; Ok(Pattern::Literal(*v)) }
        Some(Token::Underscore) => { *i += 1; Ok(Pattern::Wildcard) }
        Some(Token::Ident(name)) => {
            *i += 1;
            // Check if it starts with uppercase -> Constructor
            if name.chars().next().unwrap().is_uppercase() {
                // Constructor
                 if let Some(Token::LParen) = tokens.get(*i) {
                    *i += 1;
                    let mut pats = Vec::new();
                    loop {
                        match tokens.get(*i) {
                            Some(Token::RParen) => { *i += 1; break; }
                            _ => {
                                pats.push(parse_pattern(tokens, i)?);
                                match tokens.get(*i) {
                                    Some(Token::Comma) => { *i += 1; continue; }
                                    Some(Token::RParen) => { *i += 1; break; }
                                    _ => return Err(Error::Parse("expect , or ) in pattern".into())),
                                }
                            }
                        }
                    }
                    Ok(Pattern::Constructor(name.clone(), pats))
                 } else {
                    // Unit variant
                    Ok(Pattern::Constructor(name.clone(), vec![]))
                 }
            } else {
                // Variable
                Ok(Pattern::Variable(name.clone()))
            }
        }
        _ => Err(Error::Parse("unexpected token in pattern".into())),
    }
}

fn parse_stmt(tokens: &[Token], i: &mut usize) -> Result<Stmt, Error> {
    match tokens.get(*i) {
        Some(Token::If) => {
            parse_if(tokens, i)
        }
        Some(Token::While) => {
            *i += 1;
            let cond = parse_expr(tokens, i)?;
            let body = parse_block(tokens, i)?;
            Ok(Stmt::While(cond, body))
        }
        Some(Token::Loop) => {
            *i += 1;
            let body = parse_block(tokens, i)?;
            Ok(Stmt::Loop(body))
        }
        Some(Token::Break) => {
            *i += 1;
            Ok(Stmt::Break)
        }
        Some(Token::Continue) => {
            *i += 1;
            Ok(Stmt::Continue)
        }
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
            let body = parse_block(tokens, i)?;
            Ok(Stmt::ImplDef(trait_name, class_name, body))
        }
        Some(Token::Enum) => {
            *i += 1;
            let name = expect_ident(tokens, i)?;
            match tokens.get(*i) { Some(Token::LBrace) => { *i += 1; } _ => return Err(Error::Parse("expect { after enum name".into())) }
            let mut variants = Vec::new();
            loop {
                match tokens.get(*i) {
                    Some(Token::RBrace) => { *i += 1; break; }
                    Some(Token::Ident(v_name)) => {
                        let v_name = v_name.clone();
                        *i += 1;
                        let mut fields = Vec::new();
                        if let Some(Token::LParen) = tokens.get(*i) {
                            *i += 1;
                            loop {
                                match tokens.get(*i) {
                                    Some(Token::RParen) => { *i += 1; break; }
                                    Some(Token::Ident(f)) => {
                                        fields.push(f.clone());
                                        *i += 1;
                                        match tokens.get(*i) {
                                            Some(Token::Comma) => { *i += 1; continue; }
                                            Some(Token::RParen) => { *i += 1; break; }
                                            _ => return Err(Error::Parse("expect , or ) in variant fields".into())),
                                        }
                                    }
                                    _ => return Err(Error::Parse("expect identifier in variant fields".into())),
                                }
                            }
                        }
                        variants.push((v_name, fields));
                        match tokens.get(*i) {
                            Some(Token::Comma) => { *i += 1; continue; }
                            Some(Token::RBrace) => { *i += 1; break; }
                            _ => return Err(Error::Parse("expect , or } after variant".into())),
                        }
                    }
                    _ => return Err(Error::Parse("expect variant name or }".into())),
                }
            }
            Ok(Stmt::EnumDef(name, variants))
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

#[derive(PartialEq, PartialOrd, Copy, Clone)]
enum Precedence {
    None,
    Assignment, // =
    Or,
    And,
    Equality,   // ==, !=
    Comparison, // <, >, <=, >=, is, as
    Term,       // +, -
    Factor,     // *, /
    Unary,      // !, -
    Call,       // ., ()
    Primary,
}

impl Precedence {
}

fn get_precedence(token: &Token) -> Precedence {
    match token {
        Token::Eq => Precedence::Assignment,
        Token::Or => Precedence::Or,
        Token::And => Precedence::And,
        Token::DoubleEq | Token::NotEq => Precedence::Equality,
        Token::Lt | Token::Le | Token::Gt | Token::Ge | Token::Is | Token::As | Token::AsSafe => Precedence::Comparison,
        Token::Plus | Token::Minus => Precedence::Term,
        Token::Star | Token::Slash => Precedence::Factor,
        Token::Dot | Token::LParen => Precedence::Call,
        _ => Precedence::None,
    }
}

fn parse_expr(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
    parse_expr_pratt(tokens, i, Precedence::None)
}

fn parse_expr_pratt(tokens: &[Token], i: &mut usize, min_prec: Precedence) -> Result<Expr, Error> {
    let mut left = parse_prefix(tokens, i)?;

    while let Some(token) = tokens.get(*i) {
        let prec = get_precedence(token);
        if prec <= min_prec {
            break;
        }

        match token {
            Token::Eq => {
                *i += 1;
                // Assignment is right-associative, so we pass a lower precedence (None) to allow chaining
                let right = parse_expr_pratt(tokens, i, Precedence::None)?; 
                match left {
                    Expr::GetField(obj, field) => {
                        left = Expr::SetField(obj, field, Box::new(right));
                    }
                    _ => return Err(Error::Parse("Invalid assignment target".into())),
                }
            }
            Token::Or => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Or)?;
                left = Expr::Or(Box::new(left), Box::new(right));
            }
            Token::And => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::And)?;
                left = Expr::And(Box::new(left), Box::new(right));
            }
            Token::DoubleEq => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Equality)?;
                left = Expr::Eq(Box::new(left), Box::new(right));
            }
            Token::NotEq => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Equality)?;
                left = Expr::Ne(Box::new(left), Box::new(right));
            }
            Token::Lt => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Comparison)?;
                left = Expr::Lt(Box::new(left), Box::new(right));
            }
            Token::Le => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Comparison)?;
                left = Expr::Le(Box::new(left), Box::new(right));
            }
            Token::Gt => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Comparison)?;
                left = Expr::Gt(Box::new(left), Box::new(right));
            }
            Token::Ge => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Comparison)?;
                left = Expr::Ge(Box::new(left), Box::new(right));
            }
            Token::Plus => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Term)?;
                left = Expr::Add(Box::new(left), Box::new(right));
            }
            Token::Minus => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Term)?;
                left = Expr::Sub(Box::new(left), Box::new(right));
            }
            Token::Star => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Factor)?;
                left = Expr::Mul(Box::new(left), Box::new(right));
            }
            Token::Slash => {
                *i += 1;
                let right = parse_expr_pratt(tokens, i, Precedence::Factor)?;
                left = Expr::Div(Box::new(left), Box::new(right));
            }
            Token::Is => {
                *i += 1;
                let name = expect_ident(tokens, i)?;
                left = Expr::InstanceOf(Box::new(left), name);
            }
            Token::As => {
                *i += 1;
                let name = expect_ident(tokens, i)?;
                left = Expr::Cast(Box::new(left), name);
            }
            Token::AsSafe => {
                *i += 1;
                let name = expect_ident(tokens, i)?;
                left = Expr::CheckCast(Box::new(left), name);
            }
            Token::Dot => {
                *i += 1;
                let field = expect_ident(tokens, i)?;
                left = Expr::GetField(Box::new(left), field);
            }
            Token::LParen => {
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
                left = Expr::Call(Box::new(left), args);
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_prefix(tokens: &[Token], i: &mut usize) -> Result<Expr, Error> {
    match tokens.get(*i) {
        Some(Token::Typeof) => {
            *i += 1;
            let right = parse_expr_pratt(tokens, i, Precedence::Unary)?;
            Ok(Expr::TypeOf(Box::new(right)))
        }
        Some(Token::Match) => {
            *i += 1;
            let target = parse_expr(tokens, i)?;
            match tokens.get(*i) { Some(Token::LBrace) => { *i += 1; } _ => return Err(Error::Parse("expect { after match target".into())) }
            let mut branches = Vec::new();
            loop {
                match tokens.get(*i) {
                    Some(Token::RBrace) => { *i += 1; break; }
                    _ => {
                        let pat = parse_pattern(tokens, i)?;
                        match tokens.get(*i) { Some(Token::Arrow) => { *i += 1; } _ => return Err(Error::Parse("expect => after pattern".into())) }
                        
                        let body = if let Some(Token::LBrace) = tokens.get(*i) {
                             parse_block(tokens, i)?
                        } else {
                             let stmt = parse_stmt(tokens, i)?;
                             vec![stmt]
                        };
                        
                        branches.push((pat, body));
                        
                        if let Some(Token::Comma) = tokens.get(*i) { *i += 1; }
                    }
                }
            }
            Ok(Expr::Match(Box::new(target), branches))
        }
        Some(Token::Not) => {
            *i += 1;
            let right = parse_expr_pratt(tokens, i, Precedence::Unary)?;
            Ok(Expr::Not(Box::new(right)))
        }
        Some(Token::Minus) => {
            *i += 1;
            let right = parse_expr_pratt(tokens, i, Precedence::Unary)?;
            Ok(Expr::Neg(Box::new(right)))
        }
        Some(Token::Int(v)) => { *i += 1; Ok(Expr::Int(*v)) }
        Some(Token::True) => { *i += 1; Ok(Expr::Bool(true)) }
        Some(Token::False) => { *i += 1; Ok(Expr::Bool(false)) }
        Some(Token::Ident(name)) => {
            *i += 1;
            Ok(Expr::Variable(name.clone()))
        }
        Some(Token::New) => {
            *i += 1;
            let name = expect_ident(tokens, i)?;
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
