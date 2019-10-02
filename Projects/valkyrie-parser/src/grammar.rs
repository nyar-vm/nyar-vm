use crate::pest_parser::{Rule, Valkyrie};
use pest::Parser;

#[test]
pub fn main() {
    let pairs = Valkyrie::parse(Rule::program, ";").unwrap_or_else(|e| panic!("{}", e));

    // Because ident_list is silent, the iterator will contain idents
    for pair in pairs {
        // A pair is a combination of the rule which matched and a span of input
        println!("Rule:    {:?}", pair.as_rule());
        println!("Span:    {:?}", pair.as_span());
        println!("Text:    {}\n", pair.as_str());
        // A pair can be converted to an iterator of the tokens which make it up:
    }
    assert_eq!(0, 1)
}
