use crate::pest_parser::{Rule, Valkyrie};
use pest::Parser;

pub fn get_statements(text: &str) {
    let pairs = Valkyrie::parse(Rule::program, text).unwrap_or_else(|e| panic!("{}", e));

    // Because ident_list is silent, the iterator will contain idents
    for pair in pairs {
        // A pair is a combination of the rule which matched and a span of INPUT
        println!("Rule:    {:?}", pair.as_rule());
        println!("Span:    {:?}", pair.as_span());
        println!("Text:    {}\n", pair.as_str());
        // A pair can be converted to an iterator of the tokens which make it up:
    }
}

const INPUT: &str = r#"
if a {}
if a {} else {}
if a {} else if b {}
"#;

#[test]
fn debug() {
    get_statements(INPUT);
    assert_eq!(0, 1)
}
