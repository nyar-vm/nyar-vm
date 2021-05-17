use super::*;

mod test_assign;
// mod test_dict;
mod test_import;

const INPUT1: &str = r#"
if a {1}
if a {1} else {2}
if a {1} else if b {2}
if a {1} else if b {2} else {3}
if a {1} else if b {2} else if c {3}
if a {1} else if b {2} else if c {3} else {4}
"#;

#[test]
fn debug_if() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INPUT1);
    ast.save("tests/test_stmt/debug_if.clj")
}

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
