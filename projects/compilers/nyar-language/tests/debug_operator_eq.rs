use nyar_language::ValkyrieCompiler;

#[test]
fn generic_eq_operator_resolves() {
    let source = r#"
trait Iterator { type Item; micro has_next(self) -> bool; micro next(self) -> Option<Item>; }
unite Option<T> { Some { value: T }, None }
imply Option<T> {
    micro unwrap(self) -> T { match self { case Some(value): value case None: panic("x") } }
}
micro contains<I, T>(self: I, value: T): bool
    where I: Iterator<Item = T>
{
    let mut iter: I = self
    while iter.has_next() {
        if iter.next().unwrap() == value {
            return true
        }
    }
    return false
}
"#;
    ValkyrieCompiler::default().compile_source(source).expect("generic == in contains must resolve");
}
