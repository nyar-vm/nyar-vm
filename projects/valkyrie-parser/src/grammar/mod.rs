use pest::{iterators::Pair, Parser};

use crate::{
    debug_cases,
    grammar::parser::ValkyrieParser,
    utils::{trim_first_last, TokenExtension},
    Rule,
};
pub use context::ParsingContext;
use nyar_error::{NyarError, Result, Span};
use nyar_hir::{
    ast::{StringTemplateBuilder, Symbol},
    ASTKind, ASTNode,
};
pub use operators::PREC_CLIMBER;
use pest::prec_climber::{
    Assoc::{Left, Right},
    Operator, PrecClimber,
};

pub(crate) mod context;
pub(crate) mod control_flow;
pub(crate) mod data;
pub(crate) mod define;
pub(crate) mod expression_node;
pub(crate) mod operators;
pub(crate) mod parser;

impl ParsingContext {
    pub fn get_ast(&mut self, text: &str) -> Result<ASTKind> {
        let pairs = ValkyrieParser::parse(Rule::program, text)?;
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

    fn parse_statement(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r_all = self.get_span(&pairs);
        let mut eos = true;
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::eos | Rule::WHITESPACE | Rule::EOI => continue,
                // Rule::emptyStatement => nodes.push(ASTNode::program(r)),
                Rule::import_statement => nodes.extend(self.parse_import(pair)),
                Rule::assign_statement => {
                    let s = self.parse_assign(pair);
                    nodes.extend(s.iter().cloned());
                }
                Rule::define_statement => nodes.push(self.parse_define(pair)),
                Rule::if_statement => nodes.push(self.parse_if(pair)),
                Rule::while_statement => nodes.push(self.parse_while(pair)),
                Rule::for_statement => nodes.push(self.parse_for(pair)),
                Rule::expression => match self.parse_expression(pair) {
                    (node, e) => {
                        nodes.push(node);
                        eos = e
                    }
                },
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::block(nodes, r_all);
    }

    //
    fn parse_assign(&self, pairs: Pair<Rule>) -> Vec<ASTNode> {
        // let r = self.get_span(&pairs);
        // let mut vec = vec![];
        // let mut syms = vec![];
        // let mut types = vec![];
        // let mut typing = false;
        // let mut init: Option<AST> = None;
        // for pair in pairs.into_inner() {
        //     match pair.as_rule() {
        //         Rule::Set | Rule::Colon | Rule::Comma => continue,
        //         Rule::Let | Rule::WHITESPACE => continue,
        //         Rule::type_expr => {
        //             typing = true;
        //             for inner in pair.into_inner() {
        //                 match inner.as_rule() {
        //                     Rule::Comma => (),
        //                     Rule::expr => types.push(self.parse_expr(inner)),
        //                     _ => debug_cases!(inner),
        //                 };
        //             }
        //         }
        //         Rule::assign_pair => {
        //             let mut mods = vec![];
        //             for inner in pair.into_inner() {
        //                 match inner.as_rule() {
        //                     Rule::symbol => mods.push(self.parse_symbol(inner)),
        //                     _ => debug_cases!(inner),
        //                 };
        //             }
        //             syms.push(mods)
        //         }
        //         Rule::statement => init = Some(self.parse_statement(pair)),
        //         _ => debug_cases!(pair),
        //     };
        // }
        todo!()
        // if typing == false {
        //     for mut sym in syms {
        //         let s = sym.pop().unwrap();
        //         let mut ss = vec![];
        //         for i in sym {
        //             match i {
        //                 AST::Symbol { name, scope: _ } => ss.push(name),
        //                 _ => unreachable!(),
        //             }
        //         }
        //         let typ = AST::Symbol { name: "auto".to_string(), scope: vec![] };
        //         let ast = AST::LetBinding { symbol: Box::new(s), modifiers: ss, types: Box::new(typ), annotations: None };
        //         vec.push(ast)
        //     }
        // }
        // else {
        //     for (mut sym, typ) in syms.into_iter().zip(types.into_iter()) {
        //         let s = sym.pop().unwrap();
        //         let mut ss = vec![];
        //         for i in sym {
        //             match i {
        //                 AST::Symbol { name, scope: _ } => ss.push(name),
        //                 _ => unreachable!(),
        //             }
        //         }
        //         let ast = AST::LetBinding { symbol: Box::new(s), modifiers: ss, types: Box::new(typ), annotations: None };
        //         vec.push(ast)
        //     }
        // }
        // match init {
        //     None => (),
        //     Some(i) => {
        //         let mut s = vec![];
        //         for v in vec.clone() {
        //             match v {
        //                 AST::LetBinding { symbol, .. } => s.push(*symbol),
        //                 _ => unreachable!(),
        //             }
        //         }
        //         let lhs = AST::TupleExpression(s);
        //         let ast = AST::InfixOperators { o: Box::from("="), lhs: Box::new(lhs), rhs: Box::new(i), pos };
        //         vec.push(ast)
        //     }
        // }
        // return vec;
    }

    fn parse_block(&mut self, pairs: Pair<Rule>) -> Vec<ASTNode> {
        let mut pass: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::expr => {
                    let node = self.parse_expr(pair);
                    pass.push(node);
                }
                Rule::statement => {
                    let node = self.parse_statement(pair);
                    pass.push(node);
                }
                _ => debug_cases!(pair),
            };
        }
        return pass;
    }

    fn parse_expression(&mut self, pairs: Pair<Rule>) -> (ASTNode, bool) {
        let mut base = ASTNode::default();
        let mut eos = false;
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::expr => base = self.parse_expr(pair),
                Rule::eos => eos = true,
                _ => debug_cases!(pair),
            };
        }
        return (base, eos);
    }
}
