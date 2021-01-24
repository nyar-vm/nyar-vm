pub use context::ParsingContext;
use nyar_error::Result;
use nyar_hir::{ASTKind, ASTNode};
pub use operators::PREC_CLIMBER;
use valkyrie_pest::{Pair, Parser, Rule, ValkyrieParser};

use crate::utils::unescape;

mod context;
mod operators;

macro_rules! debug_cases {
    ($i:ident) => {{
        println!("Rule::{:?}=>continue,", $i.as_rule());
        println!("Span: {:?}", $i.as_span());
        println!("Text: {}", $i.as_str());
        unreachable!();
    }};
}

impl ParsingContext {
    pub fn get_ast(&self, text: &str) -> Result<ASTKind> {
        let pairs = ValkyrieParser::parse(Rule::program, text).unwrap_or_else(|e| panic!("{}", e));
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::COMMENT | Rule::EOI => continue,
                Rule::statement => nodes.push(self.parse_statement(pair)),
                _ => debug_cases!(pair),
            };
        }
        Ok(ASTKind::Program(nodes))
    }

    fn parse_statement(&self, pairs: Pair<Rule>) -> ASTNode {
        let r_all = self.get_span(&pairs);
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            // let r = get_position(&pair);
            match pair.as_rule() {
                Rule::eos | Rule::WHITESPACE | Rule::EOI => continue,
                // Rule::emptyStatement => nodes.push(ASTNode::program(r)),
                // Rule::importStatement => nodes.push(self.parse_import(pair)),
                // Rule::assignStatement => {
                //     let s = self.parse_assign(pair);
                //     nodes.extend(s.iter().cloned());
                // }
                // Rule::if_statement => nodes.push(self.parse_if(pair)),
                // Rule::expression => nodes.push(self.parse_expression(pair)),
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::suite(nodes, r_all);
    }
}
