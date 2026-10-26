use rusty_valkyrie::ValkyrieCompiler;

#[ignore = "debug-gate: probe/debug/inspect 临时测试，非正式合同测试"]
#[test]
fn builtin_option_unwrap_on_parameter() {
    ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace demo.sample;
unite Option<T> { Some { value: T }, None }
imply Option<T> {
    micro unwrap(self, fallback: T) -> T { match self { case Some(value): value case None: fallback } }
}
micro ch(o: Option<char>, fallback: char) -> char {
    return o.unwrap(fallback)
}
"#,
        )
        .expect("builtin Option unwrap");
}
