use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "../valkyrie.pest"]
pub struct ValkyrieParser;
