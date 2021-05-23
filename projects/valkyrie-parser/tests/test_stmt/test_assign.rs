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
def function() {}

def eager function() {}
def function(self, <, >) {}
def function(input: Integer = 0) {}

def function() -> Integer {}
def function() / DivideZero {}

def eager function(mut self, <, input: Integer = 0, >, ^list: List) -> Integer / [DivideZero, ParseError] {

}
"#;

#[test]
fn debug_def() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(DEFINE);
    ast.save("tests/test_stmt/debug_def.clj")
}
