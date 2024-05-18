use nyar_language::valkyrie::resolver_staging::expand_target_templates_for_arch;

#[test]
fn transitional_text_expander_matches_arch_branch() {
    let source = r#"<% match arch %>
<% case "wasm32" %>
return 1
<% else %>
return 0
<% end match %>"#;
    let expanded = expand_target_templates_for_arch(source, "wasm32");
    assert!(!expanded.contains("<% match "));
    assert!(expanded.contains("return 1"));
    assert!(!expanded.contains("return 0"));
}
