use crate::pest_parser::Rule::controlFlow;
use crate::pest_parser::{Rule, Valkyrie};
use crate::utils::unescape;
use nyar_ast::ast::ImportStatement;
use nyar_ast::AST;
use pest::iterators::{Pair, Pairs};
use pest::Parser;

pub fn get_statements(text: &str) {
    let pairs = Valkyrie::parse(Rule::program, text).unwrap_or_else(|e| panic!("{}", e));

    // Because ident_list is silent, the iterator will contain idents
    for pair in pairs {
        match pair.as_rule() {
            Rule::EOI => continue,
            _ => {
                println!("Rule:    {:?}", pair.as_rule());
                println!("Span:    {:?}", pair.as_span());
                println!("Text:    {}\n", pair.as_str());
            }
        };
    }
}

pub fn get_ast(text: &str) -> AST {
    let pairs = Valkyrie::parse(Rule::program, text).unwrap_or_else(|e| panic!("{}", e));
    let mut nodes: Vec<AST> = vec![];
    for pair in pairs {
        let rule = pair.as_rule();
        let node = match rule {
            Rule::EOI => continue,
            Rule::emptyStatement => AST::EmptyStatement,
            Rule::importStatement => parse_import(pair.into_inner()),
            Rule::expr => parse_expression(pair.into_inner()),
            Rule::data => parse_data(pair.into_inner()),
            _ => {
                println!("Unimplemented Valkyrie Rule::{:?}", rule);
                AST::None
            }
        };
        nodes.push(node)
    }
    println!("{:?}", nodes.clone());
    return AST::Program(nodes);
}

fn parse_import(pairs: Pairs<Rule>) -> AST {
    let mut father = 0;
    for pair in pairs {
        match pair.as_rule() {
            Rule::Dot => {
                father += 1;
                continue;
            }
            Rule::use_alias => {
                let mut nodes = vec![];
                for inner in pair.into_inner() {
                    let node = match inner.as_rule() {
                        Rule::SYMBOL => inner.as_str(),
                        _ => continue,
                    };
                    nodes.push(node)
                }
                println!("{:?}", nodes);
                let a = nodes.pop().unwrap();
                let i = ImportStatement::Alias {
                    root: 0,
                    path: vec![],
                    alias: "".to_string(),
                };
                return AST::ImportStatement(i);
            }
            Rule::use_module_select => {
                println!("Rule:    {:?}", pair.as_rule());
                println!("Span:    {:?}", pair.as_span());
                println!("Text:    {}\n", pair.as_str());
                AST::None
            }
            Rule::use_module_string => {
                println!("Rule:    {:?}", pair.as_rule());
                println!("Span:    {:?}", pair.as_span());
                println!("Text:    {}\n", pair.as_str());
                AST::None
            }
            _ => continue,
        };
    }
    return AST::None;
}

fn parse_expression(pairs: Pairs<Rule>) -> AST {
    for pair in pairs {
        // A pair is a combination of the rule which matched and a span of INPUT
        println!("Rule:    {:?}", pair.as_rule());
        println!("Span:    {:?}", pair.as_span());
        println!("Text:    {}\n", pair.as_str());
        // A pair can be converted to an iterator of the tokens which make it up:
    }
    return AST::None;
}

fn parse_data(pairs: Pairs<Rule>) -> AST {
    for pair in pairs {
        let node = match pair.as_rule() {
            Rule::String => parse_string(pair.into_inner()),
            Rule::Boolean => parse_boolean(pair.into_inner()),
            Rule::Number => parse_number(pair.into_inner()),
            Rule::Byte => parse_byte(pair.into_inner()),
            _ => {
                println!("Rule:    {:?}", pair.as_rule());
                println!("Span:    {:?}", pair.as_span());
                println!("Text:    {}\n", pair.as_str());
                AST::None
            }
        };
        return node;
    }
    return AST::None;
}

fn parse_string(pairs: Pairs<Rule>) -> AST {
    let (mut h, mut t) = ("", "".to_string());
    for pair in pairs {
        match pair.as_rule() {
            Rule::SYMBOL => h = pair.as_str(),
            Rule::StringEmpty => continue,
            Rule::StringNormal => {
                for inner in pair.into_inner() {
                    match inner.as_rule() {
                        Rule::StringText => t = unescape(inner.as_str()),
                        _ => continue,
                    };
                }
            }
            Rule::StringLiteral => {
                for inner in pair.into_inner() {
                    match inner.as_rule() {
                        Rule::StringLiteralText => t = unescape(inner.as_str()),
                        _ => continue,
                    };
                }
            }
            _ => unreachable!(),
        };
    }
    return match h {
        "" => AST::String(t),
        _ => AST::StringLiteral { handler: h.to_string(), data: t },
    };
}

fn parse_number(pairs: Pairs<Rule>) -> AST {
    for pair in pairs {
        // A pair is a combination of the rule which matched and a span of INPUT
        println!("Rule:    {:?}", pair.as_rule());
        println!("Span:    {:?}", pair.as_span());
        println!("Text:    {}\n", pair.as_str());
        // A pair can be converted to an iterator of the tokens which make it up:
    }
    return AST::None;
}

fn parse_byte(pairs: Pairs<Rule>) -> AST {
    for pair in pairs {
        // A pair is a combination of the rule which matched and a span of INPUT
        println!("Rule:    {:?}", pair.as_rule());
        println!("Span:    {:?}", pair.as_span());
        println!("Text:    {}\n", pair.as_str());
        // A pair can be converted to an iterator of the tokens which make it up:
    }
    return AST::None;
}

fn parse_boolean(pairs: Pairs<Rule>) -> AST {
    for pair in pairs {
        let node = match pair.as_rule() {
            Rule::True => AST::Boolean(true),
            Rule::False => AST::Boolean(false),
            _ => unreachable!(),
        };
        return node;
    }
    return AST::None;
}
