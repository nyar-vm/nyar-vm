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
fn debug_let() {
    unimplemented!()
    // let ast: ASTKind = ASTDump::parse(BYTES);
    // ast.save(file!());
}
