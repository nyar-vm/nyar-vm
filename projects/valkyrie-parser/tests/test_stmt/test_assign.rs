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
    ast.save("tests/test_stmt/debug_let.clj")
}

const DEFINE: &str = r#"
def a(self, <, >);
def a a::def.def[](self);

"#;

#[test]
fn debug_def() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(DEFINE);
    ast.save("tests/test_stmt/debug_def.clj")
}
