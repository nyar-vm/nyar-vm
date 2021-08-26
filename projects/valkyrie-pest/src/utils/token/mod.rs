use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter},
    iter::FromIterator,
};

use pest::iterators::{Pair, Pairs};

use nyar_error::{NyarError, Result, Span};

use crate::Rule;

pub struct Rules {
    rules: BTreeSet<Rule>,
}

impl From<Rule> for Rules {
    fn from(r: Rule) -> Self {
        Self { rules: BTreeSet::from_iter([r].iter().cloned()) }
    }
}

#[derive(Debug, Clone)]
pub struct Token<'i> {
    pub(crate) pair: Pair<'i, Rule>,
    pub rule: Rule,
    pub text: &'i str,
    pub span: Span,
    pub file_id: u32,
}

impl<'i> Display for Token<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:?}", self.into_iter().map(|f| f.rule).collect::<Vec<_>>())
    }
}

pub struct Tokens<'i> {
    pairs: Pairs<'i, Rule>,
    file_id: u32,
}

impl<'i> Token<'i> {
    pub fn new(pair: Pair<'i, Rule>, file: u32) -> Self {
        let span = Span::from_pair(&pair, file);
        Self { rule: pair.as_rule(), text: pair.as_str(), file_id: file, span, pair }
    }
    #[track_caller]
    pub fn debug_cases(&self) -> Result<!> {
        let msg = format!("Rule::{:?}=>continue,\nSpan: {:?}\nText: {}", self.rule, self.span, self.text);
        if cfg!(debug_assertions) {
            println!("{}", msg);
            unreachable!();
        }
        else {
            Err(NyarError::msg(msg))
        }
    }
    #[track_caller]
    pub fn first(&self) -> Result<Self> {
        let item = match self.pair.clone().into_inner().next() {
            Some(s) => s,
            None => {
                if cfg!(debug_assertions) {
                    unreachable!();
                }
                else {
                    return Err(NyarError::msg("first element not found"));
                }
            }
        };
        try { Self::new(item, self.file_id) }
    }
    #[track_caller]
    pub fn last(&self) -> Result<Self> {
        let item = match self.pair.clone().into_inner().next_back() {
            Some(s) => s,
            None => {
                if cfg!(debug_assertions) {
                    unreachable!();
                }
                else {
                    return Err(NyarError::msg("last element not found"));
                }
            }
        };
        try { Self::new(item, self.file_id) }
    }
    #[track_caller]
    pub fn nth(&self, id: isize) -> Result<Self> {
        let mut iter = self.pair.clone().into_inner();
        let item = match id {
            _ if id > 0 => iter.nth((id - 1) as usize),
            _ if id < 0 => iter.nth_back((-id - 1) as usize),
            _ => iter.nth(0),
        };
        let item = match item {
            Some(s) => s,
            None => {
                if cfg!(debug_assertions) {
                    unreachable!();
                }
                else {
                    return Err(NyarError::msg("nth element not found"));
                }
            }
        };
        try { Self::new(item, self.file_id) }
    }

    pub fn filter_node<R>(&self, rules: R) -> Vec<Self>
    where
        R: Into<Rules>,
    {
        let rules = rules.into().rules;
        let tokens = self.clone().into_iter();
        let mut out = vec![];
        for token in tokens {
            if rules.contains(&token.rule) {
                out.push(token)
            }
        }
        out
    }

    pub fn text_inner(&self) -> &str {
        let mut chars = self.text.chars();
        chars.next();
        chars.next_back();
        chars.as_str()
    }
}

impl<'i> Tokens<'i> {
    pub fn new(pairs: Pairs<'i, Rule>, file: u32) -> Self {
        Self { pairs, file_id: file }
    }
}

impl<'a, 'i> IntoIterator for &'a Token<'i> {
    type Item = Token<'i>;
    type IntoIter = Tokens<'i>;

    fn into_iter(self) -> Self::IntoIter {
        Tokens { pairs: self.pair.clone().into_inner(), file_id: self.file_id }
    }
}

impl<'i> Iterator for Tokens<'i> {
    type Item = Token<'i>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.pairs.next()?;
        let span = Span::from_pair(&item, self.file_id);
        try { Token { rule: item.as_rule(), text: item.as_str(), span, pair: item, file_id: self.file_id } }
    }
}
