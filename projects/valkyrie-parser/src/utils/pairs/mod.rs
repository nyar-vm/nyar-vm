use std::{collections::BTreeSet, iter::FromIterator};

use crate::{ParsingContext, Rule};
use gen_iter::gen_iter;
use pest::iterators::{Pair, Pairs};

struct Rules {
    rules: BTreeSet<Rule>,
}

impl From<Rule> for Rules {
    fn from(r: Rule) -> Self {
        Self { rules: BTreeSet::from_iter([r].iter().cloned()) }
    }
}

impl ParsingContext {}

pub trait TokenExtension<'i> {
    fn filter_node<R>(&self, rules: R) -> Box<dyn Iterator<Item = Pair<'i, Rule>>>
    where
        R: Into<Rules>,
    {
        unimplemented!("{:?}", rules.into().rules)
    }
}

impl<'i> TokenExtension<'i> for Pair<'i, Rule> {
    fn filter_node<R>(&self, rules: R) -> Box<dyn Iterator<Item = Pair<'i, Rule>>>
    where
        R: Into<Rules>,
    {
        let rules = rules.into().rules;
        gen_iter!({
            for pair in self.clone().into_inner() {
                if rules.contains(&pair.as_rule()) {
                    yield pair;
                }
            }
        })
    }
}
