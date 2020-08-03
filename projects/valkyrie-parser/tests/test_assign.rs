extern crate nyar_valkyrie;

use nyar_valkyrie::get_ast;

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
    let ast = get_ast(LET);
    ast.save("tests/debug_let.json");
}
