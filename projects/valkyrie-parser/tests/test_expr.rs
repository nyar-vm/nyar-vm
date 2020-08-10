use valkyrie_parser::{ast::ASTKind, ASTDump};

const INPUT: &str = r#"
+1+2*3^-4!!
1 +++ 2
1 * (2 + 3)
"#;

#[test]
fn debug_expr() {
    let ast: ASTKind = ASTDump::parse(BYTES);
    ast.save(file!());
}

const LIST_OR_SLICE: &str = r#"
[1 + 2];
[1 + 2, 3,];
[1 + 2, 3, []];
[1 + 2, 3, [4,5]];
[1 + 2, 3, [4,5,[]]];

a[1 + 1];
b[1 + 1] + 1;
c[1,2,3];
d[[1,2,3]];
e[1:2:3,[1,2,3]];
"#;

#[test]
fn debug_list_or_slice() {
    let ast: ASTKind = ASTDump::parse(BYTES);
    ast.save("tests/debug_list_or_slice.yaml");
}

const BRACKETS: &str = r#"
a(1)[2]
b[1](2)

Persion(20,"2",a, a: 2)

"#;

#[test]
fn debug_expr_brackets() {
    let ast: ASTKind = ASTDump::parse(BYTES);
    ast.save("tests/debug_expr_brackets.yaml");
}

const DOT_CALL: &str = r#"
a::b::c
a::b::c()

a::b.c
a::b.c()

a.b::c
a.b::c()

a.b.c
a.(b.c)
a.b.c()
a.b().c()
a().b().c()
"#;

#[test]
fn debug_dot_call() {
    let ast: ASTKind = ASTDump::parse(BYTES);
    ast.save("tests/debug_dot_call");
}
