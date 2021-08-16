use std::str::FromStr;

use nyar_hir::ast::{DecimalLiteral, IntegerLiteral, KVPair, TableExpression, XMLTemplateBuilder};

use super::*;

impl ParsingContext {
    pub(crate) fn parse_data(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        match self.try_parse_data(pairs) {
            Ok(o) => o,
            Err(e) => {
                self.push_error(e);
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
            Rule::Complex => self.parse_complex_number(pair),
            Rule::Integer => self.parse_integer(pair).as_node(r),
            Rule::Decimal => self.parse_decimal(pair).as_node(r),
            Rule::Byte => self.parse_byte(pair),
            Rule::Symbol => ASTNode::symbol(self.parse_symbol(pair), r),
            Rule::namepath => ASTNode::symbol(self.parse_namepath(pair), r),
            Rule::tuple => self.parse_tuple(pair),
            Rule::table => self.parse_table(pair),
            Rule::block => ASTNode::block(self.parse_block(pair), r),
            Rule::XML => self.parse_xml(pair),
            _ => debug_cases!(pair),
        };
        Ok(value)
    }

    fn parse_table(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut table = TableExpression::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::expr => table.push_node(self.parse_expr(pair)),
                Rule::table_pair => {
                    let r = self.get_span(&pair);
                    match self.parse_kv(pair) {
                        Ok(pair) => table.push_pair(pair.key, pair.value, r),
                        Err(e) => self.push_error(e),
                    }
                }
                _ => unreachable!(),
            };
        }
        return table.as_node(r);
    }

    pub(crate) fn parse_kv(&mut self, pairs: Pair<Rule>) -> Result<KVPair> {
        let mut pairs = pairs.into_inner();
        let (key, value) = unsafe {
            let k = pairs.next().unwrap_unchecked();
            let v = pairs.next().unwrap_unchecked();
            (k, self.parse_expr(v))
        };
        let s = self.get_span(&key);
        let pair = match key.as_rule() {
            Rule::Integer => KVPair::new(self.parse_integer(key).as_node(s), value),
            Rule::Symbol => KVPair::new(self.parse_symbol(key).as_node(s), value),
            Rule::String => {
                let symbol = Symbol::atom(trim_first_last(key.as_str())).as_node(s);
                KVPair::new(symbol, value)
            }
            _ => unreachable!(),
        };
        Ok(pair)
    }

    fn parse_tuple(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let tuple = pairs.into_inner().filter(|f| f.as_rule() == Rule::expr).map(|pair| self.parse_expr(pair)).collect();
        ASTNode::tuple(tuple, r)
    }
    fn parse_complex_number(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut pairs = pairs.into_inner();
        unsafe {
            let n = pairs.next().unwrap_unchecked();
            let h = pairs.next().unwrap_unchecked().as_str();
            match n.as_rule() {
                Rule::Integer => self.parse_integer(n).with_handler(h).as_node(r),
                Rule::Decimal => self.parse_decimal(n).with_handler(h).as_node(r),
                _ => unreachable!(),
            }
        }
    }
    pub fn parse_integer(&mut self, pairs: Pair<Rule>) -> IntegerLiteral {
        let r = self.get_span(&pairs);
        match IntegerLiteral::from_str(pairs.as_str()) {
            Ok(o) => o,
            Err(e) => {
                self.push_error(e.with_span(r));
                Default::default()
            }
        }
    }
    fn parse_decimal(&mut self, pairs: Pair<Rule>) -> DecimalLiteral {
        let r = self.get_span(&pairs);
        match DecimalLiteral::from_str(pairs.as_str()) {
            Ok(o) => o,
            Err(e) => {
                self.push_error(e.with_span(r));
                Default::default()
            }
        }
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

    pub(crate) fn parse_symbol(&self, pairs: Pair<Rule>) -> Symbol {
        let pair = pairs.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::SYMBOL_XID => Symbol::atom(pair.as_str()),
            Rule::SYMBOL_ESCAPE => Symbol::atom(trim_first_last(pair.as_str())),
            _ => unreachable!(),
        }
    }
    pub(crate) fn parse_modifiers(&self, pairs: Pair<Rule>) -> Vec<String> {
        pairs.into_inner().filter(|f| f.as_rule() == Rule::Symbol).map(|pair| self.parse_symbol(pair).name).collect()
    }
    pub(crate) fn parse_namepath(&self, pairs: Pair<Rule>) -> Symbol {
        let mut out = vec![];
        for i in pairs.into_inner() {
            match i.as_rule() {
                Rule::DOT | Rule::Proportion => continue,
                Rule::Symbol => out.push(self.parse_symbol(i)),
                Rule::use_special => out.push(Symbol::atom(i.as_str())),
                _ => debug_cases!(i),
            }
        }
        Symbol::join(out)
    }
}

impl ParsingContext {
    pub(crate) fn parse_string(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let mut builder = StringTemplateBuilder::new(self.get_span(&pairs));
        for pair in pairs.into_inner() {
            let r = self.get_span(&pair);
            if let Err(e) = self.parse_string_item(pair, &mut builder, r) {
                self.push_error(e.with_span(r))
            }
        }
        builder.as_node()
    }
    fn parse_string_item(&mut self, pair: Pair<Rule>, builder: &mut StringTemplateBuilder, r: Span) -> Result<()> {
        if builder.has_handler() {
            builder.push_buffer(pair.as_str());
            return Ok(());
        }
        match pair.as_rule() {
            // Rule::Symbol => builder.push_handler(pair.as_str()),
            Rule::STRING_SLOT => builder.push_expression(self.string_slot(pair)),
            Rule::any => builder.push_character(pair.as_str(), r)?,
            Rule::STRING_UNICODE => builder.push_unicode(pair.as_str(), r)?,
            Rule::STRING_ESCAPE => builder.push_escape(pair.as_str(), r)?,
            Rule::namepath => builder.push_symbol(self.parse_namepath(pair), r),
            // Rule::expression => builder.push_expression(self.parse_expression(pair).0),
            _ => debug_cases!(pair), // _ => unreachable!(),
        };
        Ok(())
    }
    fn string_slot(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let expr = pairs.into_inner().filter(|pair| pair.as_rule() == Rule::expr_inline).next().unwrap();
        self.parse_expr(expr)
    }
}

impl ParsingContext {
    pub(crate) fn parse_xml(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let mut builder = XMLTemplateBuilder::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::XML_TEXT => continue,
                _ => debug_cases!(pair), // _ => unreachable!(),
            }
        }
        ASTNode::default()
    }
}
