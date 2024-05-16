use nyar_language::{transitional::guest_scripts::bash::BashScript, *};

#[test]
fn collects_top_level_functions() {
    let script = BashScript::parse("greet() { echo hi; }\necho done").expect("parse");
    let bridge = BashSemanticBridge::from_script(script);
    assert_eq!(bridge.exported_symbols(), &["greet".to_string()]);
}
