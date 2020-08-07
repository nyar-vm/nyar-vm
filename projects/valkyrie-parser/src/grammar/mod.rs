mod context;
mod operators;

pub use context::LexerContext;
pub use operators::PREC_CLIMBER;

use valkyrie_pest::{Pair, Parser, Rule, ValkyrieParser};
use nyar_hir::{ASTNode, ASTKind};

macro_rules! debug_cases {
    ($i:ident) => {{
        println!("Rule::{:?}=>continue,", $i.as_rule());
        println!("Span: {:?}", $i.as_span());
        println!("Text: {}", $i.as_str());
        unreachable!();
    }};
}

impl LexerContext {
    pub fn get_ast(&self, text: &str) -> ASTKind {
        let pairs = ValkyrieParser::parse(Rule::program, text).unwrap_or_else(|e| panic!("{}", e));
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::EOI => continue,
                Rule::statement => nodes.push(self.parse_statement(pair)),
                _ => unreachable!(),
            };
        }
        return ASTKind::Program(nodes);
    }

    fn parse_statement(&self, pairs: Pair<Rule>) -> ASTNode {
        let mut nodes: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::eos | Rule::WHITESPACE | Rule::EOI => continue,
                Rule::emptyStatement => nodes.push(AST::EmptyStatement),
                Rule::importStatement => nodes.push(self.parse_import(pair)),
                Rule::assignStatement => {
                    let s = self.parse_assign(pair);
                    nodes.extend(s.iter().cloned());
                }
                Rule::if_statement => nodes.push(self.parse_if(pair)),
                Rule::expression => nodes.push(self.parse_expression(pair)),
                _ => debug_cases!(pair),
            };
        }
        return AST::Suite(nodes);
    }
    //
    // fn parse_import(&self, pairs: Pair<Rule>) -> ASTNode {
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
    // }
    //
    // fn parse_assign(&self, pairs: Pair<Rule>) -> Vec<AST> {
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
    // }
    //
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
    //
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
    //
    // fn parse_expression(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let pos = get_position(pairs.as_span());
    //     let mut base = AST::None;
    //     let mut eos = false;
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::WHITESPACE => continue,
    //             Rule::expr => base = self.parse_expr(pair),
    //             Rule::eos => eos = true,
    //             _ => debug_cases!(pair),
    //         };
    //     }
    //     return AST::Expression { base: Box::new(base), eos, pos, annotations: None };
    // }
    //
    // #[rustfmt::skip]
    // fn parse_expr(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let pos = get_position(pairs.as_span());
    //     PREC_CLIMBER.climb(
    //         pairs.into_inner(),
    //         |pair: Pair<Rule>| match pair.as_rule() {
    //             Rule::WHITESPACE => AST::EmptyStatement,
    //             Rule::expr => self.parse_expr(pair),
    //             Rule::term => self.parse_term(pair),
    //             Rule::bracket_call => debug_cases!(pair),
    //             _ => debug_cases!(pair),
    //         },
    //         |left: AST, op: Pair<Rule>, right: AST| match op.as_rule() {
    //             _ => AST::InfixOperators {
    //                 o: Box::from(op.as_str()),
    //                 lhs: Box::new(left),
    //                 rhs: Box::new(right),
    //                 pos,
    //             },
    //         },
    //     )
    // }
    //
    // fn parse_term(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let pos = get_position(pairs.as_span());
    //     let mut base = AST::None;
    //     let mut prefix = vec![];
    //     let mut suffix = vec![];
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::WHITESPACE => continue,
    //             Rule::node => base = self.parse_node(pair),
    //             Rule::Prefix => prefix.push(pair.as_str().to_string()),
    //             Rule::Suffix => suffix.push(pair.as_str().to_string()),
    //             _ => unreachable!(),
    //         };
    //     }
    //     return if prefix.len() + suffix.len() == 0 {
    //         base
    //     }
    //     else {
    //         AST::UnaryOperators { base: Box::new(base), prefix, suffix, pos }
    //     };
    // }
    //
    // fn parse_node(&self, pairs: Pair<Rule>) -> ASTNode {
    //     for pair in pairs.into_inner() {
    //         return match pair.as_rule() {
    //             Rule::bracket_call => self.parse_bracket_call(pair),
    //             Rule::expr => self.parse_expr(pair),
    //             Rule::data => self.parse_data(pair),
    //             Rule::tuple => self.parse_tuple(pair),
    //             _ => debug_cases!(pair),
    //         };
    //     }
    //     return AST::None;
    // }
    //
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
    // fn parse_data(&self, pairs: Pair<Rule>) -> ASTNode {
    //     for pair in pairs.into_inner() {
    //         let node = match pair.as_rule() {
    //             Rule::String => self.parse_string(pair),
    //             Rule::Boolean => self.parse_boolean(pair),
    //             Rule::Number => self.parse_number(pair),
    //             Rule::Byte => self.parse_byte(pair),
    //             Rule::Symbol => self.parse_symbol(pair),
    //             Rule::list => self.parse_list(pair),
    //             _ => debug_cases!(pair),
    //         };
    //         return node;
    //     }
    //     return AST::None;
    // }
    //
    // fn parse_list(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let mut vec: Vec<AST> = vec![];
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::Comma => (),
    //             Rule::expr => vec.push(self.parse_expr(pair)),
    //             _ => unreachable!(),
    //         };
    //     }
    //     return AST::ListExpression(vec);
    // }
    //
    // fn parse_tuple(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let mut vec: Vec<AST> = vec![];
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::Comma => (),
    //             Rule::expr => vec.push(self.parse_expr(pair)),
    //             _ => unreachable!(),
    //         };
    //     }
    //     return AST::TupleExpression(vec);
    // }
    //
    // fn parse_string(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let (mut h, mut t) = Default::default();
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::SYMBOL => h = pair.as_str(),
    //             Rule::StringEmpty => continue,
    //             Rule::StringNormal => {
    //                 for inner in pair.into_inner() {
    //                     match inner.as_rule() {
    //                         Rule::StringText => t = unescape(inner.as_str()),
    //                         _ => continue,
    //                     };
    //                 }
    //             }
    //             Rule::StringLiteral => {
    //                 for inner in pair.into_inner() {
    //                     match inner.as_rule() {
    //                         Rule::StringLiteralText => t = unescape(inner.as_str()),
    //                         _ => continue,
    //                     };
    //                 }
    //             }
    //             _ => unreachable!(),
    //         };
    //     }
    //     if self.refine { string_refine(h, &t) } else { Self::new_string(h, &t) }
    // }
    //
    // fn parse_number(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let (mut h, mut t) = ("", String::new());
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::Integer => {
    //                 h = "int";
    //                 t = pair.as_str().to_string();
    //             }
    //             Rule::Decimal => {
    //                 h = "fp";
    //                 t = pair.as_str().to_string();
    //             }
    //             Rule::DecimalBad => {
    //                 h = "fp";
    //                 let s = pair.as_str();
    //                 if s.starts_with('.') { t = "0".to_string() + s } else { t = s.to_string() + "0" }
    //             }
    //             Rule::Complex => {
    //                 for inner in pair.into_inner() {
    //                     match inner.as_rule() {
    //                         Rule::Integer => t = inner.as_str().to_string(),
    //                         Rule::Decimal => t = inner.as_str().to_string(),
    //                         Rule::SYMBOL => h = inner.as_str(),
    //                         _ => unreachable!(),
    //                     };
    //                 }
    //             }
    //             _ => unreachable!(),
    //         };
    //     }
    //     if self.refine { number_refine(h, &t) } else { Self::new_number(h, &t) }
    // }
    //
    // fn parse_byte(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let (mut h, mut t) = ("", "0");
    //     for pair in pairs.into_inner() {
    //         match pair.as_rule() {
    //             Rule::Byte_HEX => {
    //                 let s = pair.as_str();
    //                 h = "x0";
    //                 t = &s[2..s.len()];
    //             }
    //             Rule::Byte_OCT => {
    //                 let s = pair.as_str();
    //                 h = "o0";
    //                 t = &s[2..s.len()];
    //             }
    //             Rule::Byte_BIN => {
    //                 let s = pair.as_str();
    //                 h = "b0 ";
    //                 t = &s[2..s.len()];
    //             }
    //             _ => unreachable!(),
    //         };
    //     }
    //     if self.refine { number_refine(h, t) } else { Self::new_number(h, t) }
    // }
    //
    // fn parse_boolean(&self, pairs: Pair<Rule>) -> ASTNode {
    //     for pair in pairs.into_inner() {
    //         let node = match pair.as_rule() {
    //             Rule::True => AST::Boolean(true),
    //             Rule::False => AST::Boolean(false),
    //             _ => unreachable!(),
    //         };
    //         return node;
    //     }
    //     return AST::None;
    // }
    //
    // fn parse_symbol(&self, pairs: Pair<Rule>) -> ASTNode {
    //     let mut scope = vec![];
    //     match pairs.as_rule() {
    //         Rule::SYMBOL => scope.push(pairs.as_str().to_string()),
    //         _ => {
    //             for pair in pairs.into_inner() {
    //                 match pair.as_rule() {
    //                     Rule::SYMBOL => scope.push(pair.as_str().to_string()),
    //                     Rule::namespace => {
    //                         for inner in pair.into_inner() {
    //                             match inner.as_rule() {
    //                                 Rule::Proportion => (),
    //                                 Rule::SYMBOL => scope.push(inner.as_str().to_string()),
    //                                 _ => unreachable!(),
    //                             };
    //                         }
    //                     }
    //                     _ => unreachable!(),
    //                 };
    //             }
    //         }
    //     }
    //     let name = scope.pop().unwrap();
    //     return AST::Symbol { name, scope };
    // }
}
