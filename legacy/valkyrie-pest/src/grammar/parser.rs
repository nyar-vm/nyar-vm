use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "../valkyrie.peg"]
pub struct ValkyrieParser;
