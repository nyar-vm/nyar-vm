use std::str::FromStr;

use nyar_hir::ast::{DecimalLiteral, IntegerLiteral, KVPair, TableExpression, XMLTemplateBuilder};

use super::*;

impl ParsingContext {
    pub(crate) fn parse_data(&mut self, pairs: Token) -> ASTNode {
        match self.try_parse_data(&pairs) {
            Ok(o) => o,
            Err(e) => e.with_span(pairs.span),
        }
    }

    fn try_parse_data(&mut self, pairs: &Token) -> Result<ASTNode> {
        let pair = pairs.first()?;
        let value = match pair.rule {
            Rule::String => self.parse_string(pair),
            Rule::Special => self.parse_special(pair)?,
            Rule::Complex => self.parse_complex_number(pair)?,
            Rule::Integer => self.parse_integer(pair).as_node(pairs.span),
            Rule::Decimal => self.parse_decimal(pair).as_node(pairs.span),
            Rule::Byte => self.parse_byte(pair),
            Rule::Symbol => ASTNode::symbol(self.parse_symbol(pair)?, pairs.span),
            Rule::namepath => ASTNode::symbol(self.parse_namepath(pair)?, pairs.span),
            Rule::tuple => self.parse_tuple(pair),
            Rule::table => self.parse_table(pair),
            Rule::block => ASTNode::block(self.parse_block(pair)?, pairs.span),
            Rule::XML => self.parse_xml(pair),
            _ => pair.debug_cases()?,
        };
        Ok(value)
    }

    fn parse_table(&mut self, pairs: Token) -> Result<ASTNode> {
        let mut table = TableExpression::default();
        for pair in &pairs {
            match pair.as_rule() {
                Rule::expr => table.push_node(self.parse_expr(pair)?),
                Rule::table_pair => {
                    let r = self.get_span(&pair);
                    match self.parse_kv(pair) {
                        Ok(pair) => table.push_pair(pair.key, pair.value, r),
                        Err(e) => self.push_error(e),
                    }
                }
                _ => pair.debug_cases()?,
            };
        }
        Ok(table.as_node(pairs.span))
    }

    pub(crate) fn parse_kv(&mut self, pairs: Token) -> Result<KVPair> {
        let key = pairs.first()?;
        let value = self.parse_expr(pairs.last()?)?;
        let s = self.get_span(&key);
        let pair = match key.rule {
            Rule::Integer => KVPair::new(self.parse_integer(key).as_node(s), value),
            Rule::Symbol => KVPair::new(self.parse_symbol(key).as_node(s), value),
            Rule::String => {
                let symbol = Symbol::atom(key.text_inner()).as_node(s);
                KVPair::new(symbol, value)
            }
            _ => key.debug_cases()?,
        };
        Ok(pair)
    }
    fn parse_tuple(&mut self, pairs: Token) -> ASTNode {
        let mut tuple = vec![];
        for pair in pairs.filter_node(Rule::expr) {
            match self.parse_expr(pair) {
                Ok(o) => tuple.push(o),
                Err(e) => self.push_error(e),
            }
        }
        ASTNode::tuple(tuple, pairs.span)
    }
    fn parse_complex_number(&mut self, pairs: Token) -> Result<ASTNode> {
        let n = pairs.first()?;
        let h = pairs.last().map(|f| f.text).unwrap_or("");
        try {
            match n.rule {
                Rule::Integer => self.parse_integer(n)?.with_handler(h).as_node(pairs.span),
                Rule::Decimal => self.parse_decimal(n)?.with_handler(h).as_node(pairs.span),
                _ => n.debug_cases()?,
            }
        }
    }
    pub fn parse_integer(&mut self, pairs: Token) -> Result<IntegerLiteral> {
        Ok(IntegerLiteral::from_str(pairs.text)?)
    }
    fn parse_decimal(&mut self, pairs: Token) -> Result<DecimalLiteral> {
        Ok(DecimalLiteral::from_str(pairs.text)?)
    }
    fn parse_byte(&self, pairs: Token) -> ASTNode {
        let s = pairs.text;
        let t = &s[2..s.len()];
        let h = s.chars().nth(1).unwrap();
        ASTNode::bytes(t, h, pairs.span)
    }

    fn parse_special(&self, pairs: Token) -> Result<ASTNode> {
        try {
            match pairs.text {
                "true" => ASTNode::boolean(true, pairs.span),
                "false" => ASTNode::boolean(false, pairs.span),
                _ => NyarError::msg(pairs.text),
            }
        }
    }

    pub(crate) fn parse_symbol(&self, pairs: Token) -> Result<Symbol> {
        let pair = pairs.first()?;
        try {
            match pair.rule {
                Rule::SYMBOL_XID => Symbol::atom(pair.text),
                Rule::SYMBOL_ESCAPE => Symbol::atom(pair.text_inner()),
                _ => pair.debug_cases(),
            }
        }
    }
    pub(crate) fn parse_modifiers(&self, pairs: Token) -> Vec<String> {
        self.filter_node(&pairs, Rule::Symbol).map(|pair| self.parse_symbol(pair)?.name).collect()
    }
    pub(crate) fn parse_namepath(&self, pairs: Token) -> Result<Symbol> {
        let mut out = vec![];
        for pair in &pairs {
            match pair.rule {
                Rule::DOT | Rule::Proportion => continue,
                Rule::Symbol => out.push(self.parse_symbol(pair)?),
                Rule::use_special => out.push(Symbol::atom(pair.text)),
                _ => pair.debug_cases()?,
            }
        }
        try { Symbol::join(out) }
    }
}

impl ParsingContext {
    pub(crate) fn parse_string(&mut self, pairs: Token) -> ASTNode {
        let mut builder = StringTemplateBuilder::new(self.get_span(&pairs));
        for pair in &pairs {
            let r = self.get_span(&pair);
            match self.parse_string_item(pair, &mut builder, r) {
                Ok(_) => {}
                Err(e) => self.push_error(e.with_span(r)),
            }
        }
        builder.as_node()
    }
    fn parse_string_item(&mut self, pair: Token, builder: &mut StringTemplateBuilder, r: Span) -> Result<()> {
        if builder.has_handler() {
            builder.push_buffer(pair.text);
            return Ok(());
        }
        try {
            match pair.rule {
                Rule::Symbol => builder.push_handler(pair.text),
                Rule::STRING_SLOT => builder.push_expression(self.string_slot(pair)),
                Rule::any => builder.push_character(pair.text, r)?,
                Rule::STRING_UNICODE => builder.push_unicode(pair.text, r)?,
                Rule::STRING_ESCAPE => builder.push_escape(pair.text, r)?,
                Rule::namepath => builder.push_symbol(self.parse_namepath(pair), r),
                _ => pair.debug_cases()?,
            }
        }
    }
    fn string_slot(&mut self, pairs: Token) -> Result<ASTNode> {
        self.parse_expr(pairs.first()?)
    }
}

impl ParsingContext {
    pub(crate) fn parse_xml(&mut self, pairs: Token) -> ASTNode {
        let mut builder = XMLTemplateBuilder::default();
        for pair in &pairs {
            let r = self.get_span(&pair);
            let _ = match pair.as_rule() {
                Rule::XML_TEXT => builder.push_character(pair.text, r),
                _ => pair.debug_cases()?, // _ => unreachable!(),
            };
        }
        ASTNode::default()
    }
}
