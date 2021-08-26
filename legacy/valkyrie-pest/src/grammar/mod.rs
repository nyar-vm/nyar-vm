use pest::{
    iterators::Pair,
    prec_climber::{
        Assoc::{Left, Right},
        Operator, PrecClimber,
    },
    Parser,
};

pub use context::ParsingContext;
use nyar_error::{NyarError, Result, Span};
use nyar_hir::{
    ast::{StringTemplateBuilder, Symbol},
    ASTKind, ASTNode,
};
pub use operators::PREC_CLIMBER;

use crate::{
    grammar::parser::ValkyrieParser,
    utils::{Token, Tokens},
    Rule,
};

pub(crate) mod context;
pub(crate) mod control_flow;
pub(crate) mod data;
pub(crate) mod define;
pub(crate) mod expression_node;
mod match_catch;
pub(crate) mod operators;
pub(crate) mod parser;

impl ParsingContext {
    pub fn get_ast(&mut self, text: &str) -> Result<ASTKind> {
        let result = ValkyrieParser::parse(Rule::program, text)?;
        let pairs = Tokens::new(result, self.file_id);
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs {
            match pair.rule {
                Rule::WHITESPACE | Rule::COMMENT | Rule::EOI => continue,
                Rule::statement => nodes.push(self.parse_statement(&pair)?),
                _ => pair.debug_cases()?,
            };
        }
        try { ASTKind::Program(nodes) }
    }

    fn parse_statement(&mut self, pairs: &Token) -> Result<ASTNode> {
        let mut eos = true;
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs {
            match pair.rule {
                Rule::eos | Rule::WHITESPACE | Rule::EOI => continue,
                // Rule::emptyStatement => nodes.push(ASTNode::program(r)),
                Rule::import_statement => nodes.extend(self.parse_import(&pair)),
                Rule::assign_statement => {
                    let s = self.parse_assign(pair);
                    nodes.extend(s.iter().cloned());
                }
                Rule::define_statement => nodes.push(self.parse_define(&pair)?),
                Rule::if_statement => nodes.push(self.parse_if(&pair)?),
                Rule::while_statement => nodes.push(self.parse_while(&pair)?),
                Rule::for_statement => nodes.push(self.parse_for(&pair)?),
                Rule::expr => nodes.push(self.parse_expr(&pair)?),
                Rule::match_statement => nodes.push(self.parse_match(&pair)?),
                _ => pair.debug_cases()?,
            };
        }
        return Ok(ASTNode::block(nodes, pairs.span));
    }
    //
    fn parse_assign(&self, pairs: Token) -> Vec<ASTNode> {
        // let r = self.get_span(&token);
        // let mut vec = vec![];
        // let mut syms = vec![];
        // let mut types = vec![];
        // let mut typing = false;
        // let mut init: Option<AST> = None;
        // for pair in token.into_inner() {
        //     match pair.rule {
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
        //         _ => pair.debug_cases()?,
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

    pub fn parse_block(&mut self, pairs: &Token) -> Vec<ASTNode> {
        let mut pass: Vec<ASTNode> = vec![];
        for pair in pairs {
            match pair.rule {
                Rule::expr => match self.parse_expr(&pair) {
                    Ok(o) => pass.push(o),
                    Err(e) => self.push_error(e),
                },
                Rule::statement => match self.parse_statement(&pair) {
                    Ok(o) => pass.push(o),
                    Err(e) => self.push_error(e),
                },
                _ => match pair.debug_cases() {
                    Ok(_) => {}
                    Err(e) => self.push_error(e),
                },
            };
        }
        return pass;
    }

    fn parse_expression(&mut self, pairs: &Token) -> Result<(ASTNode, bool)> {
        let mut base = ASTNode::default();
        let mut eos = false;
        for pair in pairs {
            match pair.rule {
                Rule::WHITESPACE => continue,
                Rule::expr => base = self.parse_expr(&pair)?,
                Rule::eos => eos = true,
                _ => pair.debug_cases()?,
            };
        }
        return Ok((base, eos));
    }
}
