use valkyrie_parser::{ast::ASTKind, ASTDump};

const LET: &str = r#"
let a;
let mut a;


let a: int
let (mut a, mut b):(int,int)
let mut a, mut b:(int,int)

let a = 1;

(a,b,c) = f(x)
a.b = c

"#;

#[test]
fn debug_let() {
    let ast: ASTKind = ASTDump::parse(BYTES);
    ast.save(file!());
}
