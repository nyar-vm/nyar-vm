use pest::{iterators::Pair, Parser};

pub use context::ParsingContext;
use nyar_error::{NyarError, Result, Span};
use nyar_hir::{
    ast::{Expression, StringTemplateBuilder, Symbol},
    ASTKind, ASTNode,
};
pub use operators::PREC_CLIMBER;

use crate::{
    debug_cases,
    grammar::parser::ValkyrieParser,
    utils::{trim_first_last, unescape},
    Rule,
};

pub(crate) mod context;
pub(crate) mod data;
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
            let r = self.get_span(&pair);
            match pair.as_rule() {
                Rule::eos | Rule::WHITESPACE | Rule::EOI => continue,
                // Rule::emptyStatement => nodes.push(ASTNode::program(r)),
                Rule::importStatement => nodes.push(self.parse_import(pair)),
                Rule::assignStatement => {
                    let s = self.parse_assign(pair);
                    nodes.extend(s.iter().cloned());
                }
                Rule::if_statement => nodes.push(self.parse_if(pair)),
                Rule::expression => match self.parse_expression(pair) {
                    (node, e) => {
                        nodes.push(node);
                        eos = e
                    }
                },
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::suite(nodes, r_all);
    }

    fn parse_import(&self, pairs: Pair<Rule>) -> ASTNode {
        let _r = self.get_span(&pairs);
        unimplemented!()
        //     let mut root = 0;
        //     for pair in pairs.into_inner() {
        //         match pair.as_rule() {
        //             Rule::Dot => {
        //                 root += 1;
        //                 continue;
        //             }
        //             Rule::use_alias => {
        //                 let mut nodes: Vec<String> = vec![];
        //                 for inner in pair.into_inner() {
        //                     let node = match inner.as_rule() {
        //                         Rule::SYMBOL => inner.as_str().to_string(),
        //                         _ => continue,
        //                     };
        //                     nodes.push(node)
        //                 }
        //                 let alias = nodes.pop().unwrap();
        //                 return AST::ImportStatement {
        //                     data: ImportStatement::LocalAlias { root, path: nodes, alias },
        //                     annotations: None,
        //                 };
        //             }
        //             Rule::use_module_select => debug_cases!(pair),
        //             Rule::use_module_string => debug_cases!(pair),
        //             _ => continue,
        //         };
        //     }
        //     return AST::None;
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

    fn parse_if(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut conditions: Vec<ASTNode> = vec![];
        let mut blocks: Vec<ASTNode> = vec![];
        let mut default = None;
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::If => (),
                Rule::Else => (),
                Rule::expr => conditions.push(self.parse_expr(pair)),
                Rule::block => blocks.push(self.parse_block(pair)),
                _ => unreachable!(),
            }
        }
        if conditions.len() != blocks.len() {
            default = Some(blocks.pop().unwrap())
        }
        todo!()
        // let pairs = conditions.into_iter().zip(blocks.into_iter()).collect();
        // return ASTNode::if_statement(pairs, default, r);
    }

    fn parse_block(&mut self, pairs: Pair<Rule>) -> ASTNode {
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
        return ASTNode::default();
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

    #[rustfmt::skip]
    fn parse_expr(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        // println!("{:#?}", pairs);
        PREC_CLIMBER.climb(
            pairs.into_inner().filter(|p| p.as_rule() != Rule::WHITESPACE),
            |pair: Pair<Rule>| match pair.as_rule() {
                // Rule::WHITESPACE => ASTNode::empty_statement(r),
                Rule::expr => self.parse_expr(pair),
                Rule::term => self.parse_term(pair),
                Rule::bracket_call => debug_cases!(pair),
                _ => debug_cases!(pair),
            },
            |left: ASTNode, op: Pair<Rule>, right: ASTNode| {
                left.push_infix_chain(op.as_str(), right, r)
            },
        )
    }

    fn parse_term(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut base = ASTNode::default();
        let mut prefix = vec![];
        let mut suffix = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::COMMENT => continue,
                Rule::node => base = self.parse_node(pair),
                Rule::Prefix => prefix.push(pair.as_str().to_string()),
                Rule::Suffix => suffix.push(pair.as_str().to_string()),
                _ => unreachable!(),
            };
        }
        base.push_unary_operations(&prefix, &suffix, r)
    }

    fn parse_node(&mut self, pairs: Pair<Rule>) -> ASTNode {
        for pair in pairs.into_inner() {
            return match pair.as_rule() {
                Rule::bracket_call => self.parse_bracket_call(pair),
                Rule::expr => self.parse_expr(pair),
                Rule::data => self.parse_data(pair),
                // Rule::tuple => self.parse_list_or_tuple(pair, false),
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::default();
    }

    fn parse_bracket_call(&mut self, pairs: Pair<Rule>) -> ASTNode {
        // let r = self.get_span(&pairs);
        let mut base = ASTNode::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::data => base = self.parse_data(pair),
                Rule::apply => base = ASTNode::chain_join(base, self.parse_apply(pair)),
                Rule::slice => base = ASTNode::chain_join(base, self.parse_slice(pair)),
                Rule::dot_call => continue,
                Rule::block => continue,
                _ => debug_cases!(pair),
            };
        }
        return base;
    }

    fn parse_apply(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut args = vec![];
        // let mut kv_pairs = vec![];
        // let mut types = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                // Rule::apply_kv => args.push(self.parse_kv(pair)),
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::apply_call(args, r);
    }

    fn parse_slice(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut list = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::index => list.push(self.parse_index_term(pair)),
                _ => debug_cases!(pair),
            };
        }
        ASTNode::apply_slice(&list, r)
    }

    fn parse_index_term(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let pair = pairs.into_inner().nth(0).unwrap();
        match pair.as_rule() {
            Rule::expr => self.parse_expr(pair),
            Rule::index_step => self.parse_index_step(pair),
            _ => debug_cases!(pair),
        }
    }

    fn parse_index_step(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Colon => continue,
                Rule::expr => vec.push(self.parse_expr(pair)),
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::default();
    }
}
