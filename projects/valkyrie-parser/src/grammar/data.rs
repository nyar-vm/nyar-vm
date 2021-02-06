use nyar_error::Span;
use nyar_hir::ast::{StringTemplateBuilder, Symbol};

use crate::utils::trim_first_last;

use super::*;

impl ParsingContext {
    pub(crate) fn parse_data(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let pair = pairs.into_inner().nth(0).unwrap();
        match pair.as_rule() {
            Rule::string => self.parse_string(pair),
            Rule::Special => self.parse_special(pair),
            Rule::Number => self.parse_number(pair),
            Rule::Byte => self.parse_byte(pair),
            Rule::symbol => ASTNode::symbol(self.parse_symbol(pair), r),
            Rule::list => self.parse_list_or_tuple(pair, true),
            Rule::dict => self.parse_dict(pair),
            _ => debug_cases!(pair),
        }
    }

    fn parse_dict(&self, pairs: Pair<Rule>) -> ASTNode {
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::expr => vec.push(self.parse_expr(pair)),
                Rule::key_value => {}
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::default();
    }

    fn parse_kv(&self, pairs: Pair<Rule>) -> ASTNode {
        todo!()
        // let (mut k, mut v) = (ASTNode::default(), ASTNode::default());
        // for pair in pairs.into_inner() {
        //     match pair.as_rule() {
        //         Rule::WHITESPACE | Rule::Colon => continue,
        //         Rule::symbol => k = self.parse_symbol(pair),
        //         Rule::expr => v = self.parse_expr(pair),
        //         _ => debug_cases!(pair),
        //     };
        // }
        // match k.kind {
        //     ASTKind::Nothing => k,
        //     _ => ASTNode::kv_pair(k, v),
        // }
    }

    fn parse_list_or_tuple(&self, pairs: Pair<Rule>, is_list: bool) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::Comma => continue,
                Rule::expr => vec.push(self.parse_expr(pair)),
                _ => unreachable!(),
            };
        }
        match is_list {
            true => ASTNode::list(vec, r),
            false => ASTNode::tuple(vec, r),
        }
    }

    fn parse_number(&self, pairs: Pair<Rule>) -> ASTNode {
        todo!()
        // let r = self.get_span(&pairs);
        // let (mut h, mut t, mut i) = Default::default();
        // for pair in pairs.into_inner() {
        //     match pair.as_rule() {
        //         Rule::Integer => {
        //             i = true;
        //             t = pair.as_str().to_string();
        //         }
        //         Rule::Decimal => {
        //             i = false;
        //             t = pair.as_str().to_string();
        //         }
        //         // Rule::DecimalBad => {
        //         //     // h = "dec";
        //         //     let s = pair.as_str();
        //         //     if s.starts_with('.') { t = "0".to_string() + s } else { t = s.to_string() + "0" }
        //         // }
        //         Rule::Complex => {
        //             for inner in pair.into_inner() {
        //                 match inner.as_rule() {
        //                     Rule::Integer => {
        //                         i = true;
        //                         t = inner.as_str().to_string()
        //                     }
        //                     Rule::Decimal => {
        //                         i = false;
        //                         t = inner.as_str().to_string()
        //                     }
        //                     Rule::SYMBOL => h = inner.as_str(),
        //                     _ => unreachable!(),
        //                 };
        //             }
        //         }
        //         _ => unreachable!(),
        //     };
        // }
        // ASTNode::number(h, t.as_str(), i, r)
    }

    fn parse_byte(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let s = pairs.as_str();
        let t = &s[2..s.len()];
        let h = s.chars().nth(1).unwrap();
        ASTNode::bytes(t, h, r)
    }

    fn parse_special(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        match pairs.as_str() {
            "true" => ASTNode::boolean(true, r),
            "false" => ASTNode::boolean(false, r),
            _ => unreachable!(),
        }
    }

    fn parse_symbol(&self, pairs: Pair<Rule>) -> Symbol {
        let pair = pairs.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::SYMBOL_XID => Symbol::atom(pair.as_str()),
            Rule::SYMBOL_ESCAPE => Symbol::atom(trim_first_last(pair.as_str())),
            _ => unreachable!(),
        }
    }
}

impl ParsingContext {
    fn parse_string(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut handler = "";
        // let (mut h, mut t) = Default::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::StringTemplate => {
                    return match handler.is_empty() {
                        true => self.parse_string_template(pair),
                        false => self.parse_string_raw(pair, handler, r),
                    };
                }
                Rule::StringSimple => {
                    return ASTNode::string_handler(trim_first_last(pair.as_str()), handler, r);
                }
                Rule::symbol => handler = pair.as_str(),
                _ => unreachable!(),
            };
        }
        unreachable!()
    }
    fn parse_string_raw(&self, pairs: Pair<Rule>, handler: &str, span: Span) -> ASTNode {
        let mut builder = String::with_capacity(pairs.as_str().len());
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Quotation => continue,
                Rule::Apostrophe => continue,
                Rule::StringItem => builder.push_str(pair.as_str()),
                _ => unreachable!(),
            };
        }
        ASTNode::string_handler(builder, handler, span)
    }
    fn parse_string_template(&self, pairs: Pair<Rule>) -> ASTNode {
        let mut builder = StringTemplateBuilder::new(self.get_span(&pairs));
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Quotation => continue,
                Rule::Apostrophe => continue,
                Rule::StringItem => self.parse_string_item(pair, &mut builder),
                _ => debug_cases!(pair), // _ => unreachable!(),
            };
        }
        builder.build()
    }
    fn parse_string_item(&self, pairs: Pair<Rule>, builder: &mut StringTemplateBuilder) {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::expression => self.parse_expression(pair),
                _ => debug_cases!(pair), // _ => unreachable!(),
            };
        }
    }
}
