use crate::pest_parser::{Rule, Valkyrie};
use serde_json;
use nyar_ast::AST;
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use snailquote::unescape;

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
    let (mut h, mut t) = ("n", "".to_string());
    for pair in pairs {
        match pair.as_rule() {
            Rule::SYMBOL => h = pair.as_str(),
            Rule::StringEmpty => continue,
            Rule::StringNormal => {
                for inner in pair.into_inner() {
                    match inner.as_rule() {
                        Rule::StringText => t = unescape(inner.as_str()).unwrap_or_default(),
                        _ => continue,
                    };
                }
            }
            Rule::StringLiteral => {
                for inner in pair.into_inner() {
                    match inner.as_rule() {
                        Rule::StringLiteralText => t = unescape(inner.as_str()).unwrap_or_default(),
                        _ => continue,
                    };
                }
            }
            _ => unreachable!(),
        };
    }
    return AST::StringLiteral { handler: h.to_string(), data: t };
}
