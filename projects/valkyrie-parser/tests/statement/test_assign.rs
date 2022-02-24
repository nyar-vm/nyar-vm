use super::*;

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
fn debug_let() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(LET);
    ast.save("tests/statement/debug_let.clj")
}
