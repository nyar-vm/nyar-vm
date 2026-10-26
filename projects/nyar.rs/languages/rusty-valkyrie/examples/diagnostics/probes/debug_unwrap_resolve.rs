use rusty_valkyrie::ValkyrieCompiler;

const SOURCE: &str = r#"
unite Option<T> { Some { value: T }, None }
imply Option<T> {
    micro unwrap(self) -> T { match self { case Some(value): value case None: return None } }
}
micro test(x: Option<i64>) -> i64 { return x.unwrap() }
"#;

#[ignore = "debug-gate: probe/debug/inspect 临时测试，非正式合同测试"]
#[test]
fn unwrap_on_concrete_option_resolves() {
    ValkyrieCompiler::default().compile_source(SOURCE).expect("unwrap must resolve");
}
