use super::*;

const INPUT2: &str = r#"
while {
    looping
}
while true {
    looping
}
while a > 0 {
    if (a > 0) {
        break
    }
    else {
        continue
    }
}
else {
    nothing
}
"#;

#[test]
fn debug_while() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INPUT2);
    ast.save("tests/test_stmt/debug_while.clj")
}

const INPUT3: &str = r#"
for i in j {
    looping
}
for i in j {
    looping
}
else {
    nothing
}
"#;

#[test]
fn debug_for() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INPUT3);
    ast.save("tests/test_stmt/debug_for.clj")
}
