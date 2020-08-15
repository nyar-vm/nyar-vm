mod context;
mod operators;

pub use context::LexerContext;
pub use operators::PREC_CLIMBER;

use crate::utils::{get_position, unescape};
use nyar_hir::{ASTKind, ASTNode, Result};
use valkyrie_pest::{Pair, Parser, Rule, ValkyrieParser};

macro_rules! debug_cases {
    ($i:ident) => {{
        println!("Rule::{:?}=>continue,", $i.as_rule());
        println!("Span: {:?}", $i.as_span());
        println!("Text: {}", $i.as_str());
        unreachable!();
    }};
}

impl LexerContext {
    pub fn get_ast(&self, text: &str) -> Result<ASTKind> {
        let pairs = ValkyrieParser::parse(Rule::program, text).unwrap_or_else(|e| panic!("{}", e));
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::EOI => continue,
                Rule::statement => nodes.push(self.parse_statement(pair)),
                _ => unreachable!(),
            };
        }
        Ok(ASTKind::Program(nodes))
    }

    fn parse_statement(&self, pairs: Pair<Rule>) -> ASTNode {
        let r_all = get_position(&pairs);
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            let r = get_position(&pair);
            match pair.as_rule() {
                Rule::eos | Rule::WHITESPACE | Rule::EOI => continue,
                Rule::emptyStatement => nodes.push(ASTNode::empty_statement(r)),
                // Rule::importStatement => nodes.push(self.parse_import(pair)),
                Rule::assignStatement => {
                    let s = self.parse_assign(pair);
                    nodes.extend(s.iter().cloned());
                }
                // Rule::if_statement => nodes.push(self.parse_if(pair)),
                Rule::expression => nodes.push(self.parse_expression(pair)),
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::suite(nodes, r_all);
    }
    //
    fn parse_import(&self, pairs: Pair<Rule>) -> ASTNode {
        let _r = get_position(&pairs);
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
        let _r = get_position(&pairs);
        unimplemented!()
        //     let pos = get_position(pairs.as_span());
        //     let mut vec = vec![];
        //     let mut syms = vec![];
        //     let mut types = vec![];
        //     let mut typing = false;
        //     let mut init: Option<AST> = None;
        //     for pair in pairs.into_inner() {
        //         match pair.as_rule() {
        //             Rule::Set | Rule::Colon | Rule::Comma => continue,
        //             Rule::Let | Rule::WHITESPACE => continue,
        //             Rule::type_expr => {
        //                 typing = true;
        //                 for inner in pair.into_inner() {
        //                     match inner.as_rule() {
        //                         Rule::Comma => (),
        //                         Rule::expr => types.push(self.parse_expr(inner)),
        //                         _ => debug_cases!(inner),
        //                     };
        //                 }
        //             }
        //             Rule::assign_pair => {
        //                 let mut mods = vec![];
        //                 for inner in pair.into_inner() {
        //                     match inner.as_rule() {
        //                         Rule::Symbol => mods.push(self.parse_symbol(inner)),
        //                         Rule::SYMBOL => mods.push(self.parse_symbol(inner)),
        //                         _ => debug_cases!(inner),
        //                     };
        //                 }
        //                 syms.push(mods)
        //             }
        //             Rule::statement => init = Some(self.parse_statement(pair)),
        //             _ => debug_cases!(pair),
        //         };
        //     }
        //     if typing == false {
        //         for mut sym in syms {
        //             let s = sym.pop().unwrap();
        //             let mut ss = vec![];
        //             for i in sym {
        //                 match i {
        //                     AST::Symbol { name, scope: _ } => ss.push(name),
        //                     _ => unreachable!(),
        //                 }
        //             }
        //             let typ = AST::Symbol { name: "auto".to_string(), scope: vec![] };
        //             let ast = AST::LetBinding { symbol: Box::new(s), modifiers: ss, types: Box::new(typ), annotations: None };
        //             vec.push(ast)
        //         }
        //     }
        //     else {
        //         for (mut sym, typ) in syms.into_iter().zip(types.into_iter()) {
        //             let s = sym.pop().unwrap();
        //             let mut ss = vec![];
        //             for i in sym {
        //                 match i {
        //                     AST::Symbol { name, scope: _ } => ss.push(name),
        //                     _ => unreachable!(),
        //                 }
        //             }
        //             let ast = AST::LetBinding { symbol: Box::new(s), modifiers: ss, types: Box::new(typ), annotations: None };
        //             vec.push(ast)
        //         }
        //     }
        //     match init {
        //         None => (),
        //         Some(i) => {
        //             let mut s = vec![];
        //             for v in vec.clone() {
        //                 match v {
        //                     AST::LetBinding { symbol, .. } => s.push(*symbol),
        //                     _ => unreachable!(),
        //                 }
        //             }
        //             let lhs = AST::TupleExpression(s);
        //             let ast = AST::InfixOperators { o: Box::from("="), lhs: Box::new(lhs), rhs: Box::new(i), pos };
        //             vec.push(ast)
        //         }
        //     }
        //     return vec;
    }

    // fn parse_if(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let mut conditions: Vec<AST> = vec![];
    //     let mut blocks: Vec<AST> = vec![];
    //     let mut default = None;
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::WHITESPACE => continue,
    //             Rule::If => (),
    //             Rule::Else => (),
    //             Rule::expr => conditions.push(self.parse_expr(pair)),
    //             Rule::block => blocks.push(self.parse_block(pair)),
    //             _ => unreachable!(),
    //         }
    //     }
    //     if conditions.len() != blocks.len() {
    //         default = Some(Box::new(blocks.pop().unwrap()))
    //     }
    //     let pairs = conditions.into_iter().zip(blocks.into_iter()).collect();
    //     return AST::IfStatement { pairs, default, annotations: None };
    // }

    // fn parse_dict(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let mut vec: Vec<AST> = vec![];
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::expr => vec.push(self.parse_expr(pair)),
    //             _ => debug_cases!(pair),
    //         };
    //     }
    //     return AST::None;
    // }
    //
    // fn parse_block(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let mut pass: Vec<AST> = vec![];
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::expr => {
    //                 let node = self.parse_expr(pair);
    //                 pass.push(node);
    //             }
    //             Rule::statement => {
    //                 let node = self.parse_statement(pair);
    //                 pass.push(node);
    //             }
    //             _ => debug_cases!(pair),
    //         };
    //     }
    //     return AST::None;
    // }

    fn parse_expression(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
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
        return ASTNode::expression(base, eos, r);
    }

    #[rustfmt::skip]
    fn parse_expr(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        PREC_CLIMBER.climb(
            pairs.into_inner(),
            |pair: Pair<Rule>| match pair.as_rule() {
                Rule::WHITESPACE => ASTNode::empty_statement(r),
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

    fn parse_term(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        let mut base = ASTNode::default();
        let mut prefix = vec![];
        let mut suffix = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::node => base = self.parse_node(pair),
                Rule::Prefix => prefix.push(pair.as_str().to_string()),
                Rule::Suffix => suffix.push(pair.as_str().to_string()),
                _ => unreachable!(),
            };
        }
        return if prefix.len() + suffix.len() == 0 {
            base
        }
        else {
            unimplemented!()
            //ASTNode::UnaryOperators { base: Box::new(base), prefix, suffix, r }
        };
    }

    fn parse_node(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        for pair in pairs.into_inner() {
            return match pair.as_rule() {
                // Rule::bracket_call => self.parse_bracket_call(pair),
                Rule::expr => self.parse_expr(pair),
                Rule::data => self.parse_data(pair),
                Rule::tuple => self.parse_tuple(pair),
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::default();
    }

    // fn parse_bracket_call(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let mut base = AST::None;
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::WHITESPACE => continue,
    //             Rule::data => base = self.parse_data(pair),
    //             Rule::apply => {
    //                 let apply = self.parse_apply(pair);
    //                 // return AST::ApplyExpression { base: Box::new(base), ..apply };
    //                 return apply.set_base(base);
    //             }
    //             Rule::slice => {
    //                 let mut list = vec![];
    //                 for inner in pair.into_inner() {
    //                     match inner.as_rule() {
    //                         Rule::WHITESPACE => continue,
    //                         Rule::Comma => (),
    //                         Rule::index => list.push(self.parse_index(inner)),
    //                         _ => unreachable!(),
    //                     };
    //                 }
    //                 return AST::SliceExpression { base: Box::new(base), list };
    //             }
    //             _ => debug_cases!(pair),
    //         };
    //     }
    //     return AST::None;
    // }
    //
    // fn parse_apply(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let pos = get_position(pairs.as_span());
    //     let mut args = vec![];
    //     let mut kv_pairs = vec![];
    //     let mut types = vec![];
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::WHITESPACE => continue,
    //             Rule::Comma => (),
    //             Rule::apply_kv => {
    //                 let (mut k, mut v) = (AST::None, AST::None);
    //                 for inner in pair.into_inner() {
    //                     match inner.as_rule() {
    //                         Rule::WHITESPACE | Rule::Colon => continue,
    //                         Rule::SYMBOL => k = self.parse_symbol(inner),
    //                         Rule::expr => v = self.parse_expr(inner),
    //                         _ => debug_cases!(inner),
    //                     };
    //                 }
    //                 match k {
    //                     AST::None => args.push(k),
    //                     _ => kv_pairs.push((k, v)),
    //                 }
    //             }
    //             Rule::apply => {
    //                 for inner in pair.into_inner() {
    //                     match inner.as_rule() {
    //                         Rule::expr => types.push(self.parse_expr(inner)),
    //                         _ => unreachable!(),
    //                     };
    //                 }
    //             }
    //             _ => debug_cases!(pair),
    //         };
    //     }
    //     return AST::ApplyExpression { base: Box::new(AST::None), types, args, kv_pairs, pos };
    // }
    //
    // fn parse_index(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let mut base = AST::None;
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::expr => return self.parse_expr(pair),
    //             _ => debug_cases!(pair),
    //         };
    //     }
    //     return AST::None;
    // }
    //
    fn parse_data(&self, pairs: Pair<Rule>) -> ASTNode {
        for pair in pairs.into_inner() {
            let node = match pair.as_rule() {
                Rule::String => self.parse_string(pair),
                Rule::Boolean => self.parse_boolean(pair),
                Rule::Null=>self.parse_null(pair),
                Rule::Number => self.parse_number(pair),
                Rule::Byte => self.parse_byte(pair),
                Rule::Symbol => self.parse_symbol(pair),
                Rule::list => self.parse_list(pair),
                _ => debug_cases!(pair),
            };
            return node;
        }
        return ASTNode::default();
    }

    fn parse_list(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Comma => (),
                Rule::expr => vec.push(self.parse_expr(pair)),
                _ => unreachable!(),
            };
        }
        return ASTNode::list(vec, r);
    }

    fn parse_tuple(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Comma => (),
                Rule::expr => vec.push(self.parse_expr(pair)),
                _ => unreachable!(),
            };
        }
        return ASTNode::tuple(vec, r);
    }

    fn parse_string(&self, pairs: Pair<Rule>) -> ASTNode {
        let (mut h, mut t) = Default::default();
        for pair in pairs.into_inner() {
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
        unimplemented!()
    }

    fn parse_number(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        let (mut h, mut t) = ("", String::new());
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Integer => {
                    // h = "int";
                    t = pair.as_str().to_string();
                }
                Rule::Decimal => {
                    // h = "dec";
                    t = pair.as_str().to_string();
                }
                Rule::DecimalBad => {
                    // h = "dec";
                    let s = pair.as_str();
                    if s.starts_with('.') { t = "0".to_string() + s } else { t = s.to_string() + "0" }
                }
                Rule::Complex => {
                    for inner in pair.into_inner() {
                        match inner.as_rule() {
                            Rule::Integer => t = inner.as_str().to_string(),
                            Rule::Decimal => t = inner.as_str().to_string(),
                            Rule::SYMBOL => h = inner.as_str(),
                            _ => unreachable!(),
                        };
                    }
                }
                _ => unreachable!(),
            };
        }
        ASTNode::number(h, t.as_str(), r)
    }

    fn parse_byte(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        let (mut h, mut t) = ('0', "0");
        for pair in pairs.into_inner() {
            let s = pair.as_str();
            t = &s[2..s.len()];
            h = s.chars().nth(1).unwrap();
        }
        ASTNode::bytes(h, t, r)
    }

    fn parse_boolean(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        let pair = pairs.into_inner().nth(0).unwrap();
        match pair.as_rule() {
            Rule::True => ASTNode::boolean(true, r),
            Rule::False => ASTNode::boolean(false, r),
            _ => unreachable!(),
        }
    }

    fn parse_null(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        ASTNode::null(r)
    }

    fn parse_symbol(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = get_position(&pairs);
        let mut scope = vec![];
        match pairs.as_rule() {
            Rule::SYMBOL => scope.push(pairs.as_str().to_string()),
            _ => {
                for pair in pairs.into_inner() {
                    match pair.as_rule() {
                        Rule::SYMBOL => scope.push(pair.as_str().to_string()),
                        Rule::namespace => {
                            for inner in pair.into_inner() {
                                match inner.as_rule() {
                                    Rule::Proportion => (),
                                    Rule::SYMBOL => scope.push(inner.as_str().to_string()),
                                    _ => unreachable!(),
                                };
                            }
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }
        let name = scope.pop().unwrap();
        unimplemented!()
        // return ASTNode::symbol { name, scope, r };
    }
}
