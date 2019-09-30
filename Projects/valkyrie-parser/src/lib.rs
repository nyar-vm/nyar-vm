extern crate pest;
#[macro_use]
extern crate pest_derive;

mod expand;
mod grammar;

use expand::{Rule, Valkyrie};

#[test]
fn main() {
    let pairs = Valkyrie::parse(Rule::program, "a1 b2").unwrap_or_else(|e| panic!("{}", e));

    // Because ident_list is silent, the iterator will contain idents
    for pair in pairs {
        // A pair is a combination of the rule which matched and a span of input
        println!("Rule:    {:?}", pair.as_rule());
        println!("Span:    {:?}", pair.as_span());
        println!("Text:    {}", pair.as_str());
    }
}
