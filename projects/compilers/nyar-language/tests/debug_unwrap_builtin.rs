use nyar_language::ValkyrieCompiler;

#[test]
fn builtin_option_unwrap_on_parameter() {
    ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace demo.sample;
micro ch(o: Option<char>) -> char {
    return o.unwrap()
}
"#,
        )
        .expect("builtin Option unwrap");
}
