use std::{collections::BTreeSet, iter::FromIterator};

use pest::iterators::{Pair, Pairs};

use gen_iter::gen_iter;

use crate::{ParsingContext, Rule};

pub struct Rules {
    rules: BTreeSet<Rule>,
}

pub struct Token<'i> {
    pub rule: Rule,
    pub text: &'i str,
    pub node: Pair<'i, Rule>,
}

impl From<Rule> for Rules {
    fn from(r: Rule) -> Self {
        Self { rules: BTreeSet::from_iter([r].iter().cloned()) }
    }
}

pub trait TokenExtension<'i>
where
    Self: Sized,
{
    fn first(&self) -> Option<Token<'i>>;
    fn last(&self) -> Option<Token<'i>>;
}

impl<'i> TokenExtension<'i> for Pair<'i, Rule> {
    fn first(&self) -> Option<Self> {
        let node = self.clone().into_inner().next()?;
    }
    fn last(&self) -> Option<Self> {
        self.clone().into_inner().next_back()
    }
}

impl ParsingContext {
    pub fn filter_node<'i, R>(&self, pairs: &'i Pair<'i, Rule>, rules: R) -> impl Iterator<Item = Pair<'i, Rule>> + 'i
    where
        R: Into<Rules>,
    {
        let rules = rules.into().rules;
        gen_iter!(move {
            for pair in pairs.clone().into_inner() {
                if rules.contains(&pair.as_rule()) {
                    yield pair;
                }
            }
        })
    }
}
