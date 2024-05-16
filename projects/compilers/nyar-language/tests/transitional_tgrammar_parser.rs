use nyar_language::transitional::tgrammar::{TgKeyword, TgNode, classify_directive, parse_tgrammar_fragment};

#[test]
fn end_directive_accepts_optional_label_suffix() {
    assert_eq!(classify_directive("end"), (Some(TgKeyword::End), String::new()));
    assert_eq!(classify_directive("end match"), (Some(TgKeyword::End), String::new()));
    assert_eq!(classify_directive("end if"), (Some(TgKeyword::End), String::new()));
    assert_eq!(classify_directive("end loop"), (Some(TgKeyword::End), String::new()));
}

#[test]
fn parse_match_block_closed_by_end_match() {
    let source = r#"<% match arch %>
<% case "clr" %>
clr_only()
<% else %>
other()
<% end match %>"#;
    let root = parse_tgrammar_fragment(source).expect("parse").0;
    assert_eq!(root.len(), 1);
    let TgNode::Match(match_node) = &root[0]
    else {
        panic!("expected match node");
    };
    assert_eq!(match_node.scrutinee, "arch");
    assert_eq!(match_node.arms.len(), 2);
}
