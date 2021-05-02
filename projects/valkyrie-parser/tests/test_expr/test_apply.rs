use super::*;

const BRACKETS: &str = r#"
a(1)[2]{3}
a(1){2}[3]
a[1](2){3}
a[1]{2}(3)
a{1}(2)[3]
a{1}[2](3)
"#;

#[test]
fn debug_apply() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(BRACKETS);
    ast.save("tests/test_expr/debug_apply.clj")
}

const SLICE: &str = r#"
a[1]
a[[1]]
a[2, 1 + 1]
a[1:1]
a[2:2, 1 + 1: 1 + 1]
e[1:2:3,[1,2,3]];
~a[:]
~a[::]
a[::-1]
a[:-2:]
a[2::]
"#;

#[test]
fn debug_slice() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(SLICE);
    ast.save("tests/test_expr/debug_slice.clj")
}

const INDEX: &str = r#"
a.1
a
    .1

a[1]

a   [
    2
    ]

a
[3,4]

a
[5]
[6]
"#;

#[test]
fn debug_index() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INDEX);
    ast.save("tests/test_expr/debug_index.clj")
}

const APPLY: &str = r#"
a({}) {}
"#;

#[test]
fn debug_apply2() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(APPLY);
    ast.save("tests/test_expr/debug_apply2.clj")
}
