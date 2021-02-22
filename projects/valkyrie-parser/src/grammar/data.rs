use std::str::FromStr;

use nyar_hir::ast::{DecimalLiteral, IntegerLiteral, KVPair};

use super::*;

impl ParsingContext {
    pub(crate) fn parse_data(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        match self.try_parse_data(pairs) {
            Ok(o) => o,
            Err(e) => {
                self.errors.push(e);
                ASTNode::null(r)
            }
        }
    }

    pub(crate) fn try_parse_data(&mut self, pairs: Pair<Rule>) -> Result<ASTNode> {
        let r = self.get_span(&pairs);
        let pair = pairs.into_inner().nth(0).unwrap();
        let value = match pair.as_rule() {
            Rule::String => self.parse_string(pair),
            Rule::Special => self.parse_special(pair),
            Rule::Complex => self.parse_complex_number(pair)?,
            Rule::Integer => self.parse_integer(pair)?.as_node(r),
            Rule::Decimal => self.parse_decimal(pair)?.as_node(r),
            Rule::Byte => self.parse_byte(pair),
            Rule::Symbol => ASTNode::symbol(self.parse_symbol(pair), r),
            Rule::namepath => ASTNode::symbol(self.parse_namepath(pair), r),
            Rule::tuple => self.parse_tuple(pair, false),
            Rule::table => self.parse_table(pair),
            _ => debug_cases!(pair),
        };
        Ok(value)
    }

    fn parse_table(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let mut vec: Vec<ASTNode> = vec![];
        pairs.into_inner().map(|pair| self.parse_table_item(pair))

        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::expr => vec.push(self.parse_table_item(pair)),
                Rule::key_value => {}
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::default();
    }

    fn parse_table_item(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let mut vec: Vec<ASTNode> = vec![];
        pairs.into_inner().map(|f| self.parse_table())

        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::expr => vec.push(self.parse_expr(pair)),
                Rule::key_value => {}
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::default();
    }

    fn parse_kv(&mut self, pairs: Pair<Rule>) -> KVPair {
        let r = self.get_span(&pairs);
        let (mut k, mut v) = (ASTNode::default(), ASTNode::default());
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::Colon => continue,
                Rule::Symbol => k = ASTNode::symbol(self.parse_symbol(pair), r),
                Rule::expr => v = self.parse_expr(pair),
                _ => debug_cases!(pair),
            };
        }
        ASTNode::kv_pair(k, v)
    }

    fn parse_tuple(&mut self, pairs: Pair<Rule>, is_list: bool) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::expr => vec.push(self.parse_expr(pair)),
                _ => unreachable!(),
            };
        }
        match is_list {
            true => ASTNode::list(vec, r),
            false => ASTNode::tuple(vec, r),
        }
    }
    fn parse_complex_number(&self, pairs: Pair<Rule>) -> Result<ASTNode> {
        let r = self.get_span(&pairs);
        let mut pairs = pairs.into_inner();
        unsafe {
            let n = pairs.next().unwrap_unchecked();
            let h = pairs.next().unwrap_unchecked().as_str();
            let out = match n.as_rule() {
                Rule::Integer => self.parse_integer(n)?.with_handler(h).as_node(r),
                Rule::Decimal => self.parse_decimal(n)?.with_handler(h).as_node(r),
                _ => unreachable!(),
            };
            return Ok(out);
        }
    }
    fn parse_integer(&self, pairs: Pair<Rule>) -> Result<IntegerLiteral> {
        IntegerLiteral::from_str(pairs.as_str())
    }
    fn parse_decimal(&self, pairs: Pair<Rule>) -> Result<DecimalLiteral> {
        DecimalLiteral::from_str(pairs.as_str())
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
    fn parse_namepath(&self, pairs: Pair<Rule>) -> Symbol {
        Symbol::join(
            pairs.into_inner().filter(|node| node.as_rule() == Rule::Symbol).map(|pair| self.parse_symbol(pair)).collect(),
        )
    }
}

impl ParsingContext {
    fn parse_string(&mut self, pairs: Pair<Rule>) -> ASTNode {
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
                Rule::Symbol => handler = pair.as_str(),
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
    fn parse_string_template(&mut self, pairs: Pair<Rule>) -> ASTNode {
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
    fn parse_string_item(&mut self, pairs: Pair<Rule>, builder: &mut StringTemplateBuilder) {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::expression => builder.push_expression(self.parse_expression(pair).0),
                _ => debug_cases!(pair), // _ => unreachable!(),
            };
        }
    }
}
