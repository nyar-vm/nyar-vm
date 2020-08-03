use crate::Settings;
use nyar_ast::AST;

impl Settings {
    pub(crate) fn new_number(handler: &str, data: &str) -> AST {
        AST::NumberLiteral { handler: String::from(handler), data: String::from(data) }
    }
    pub(crate) fn new_string(handler: &str, data: &str) -> AST {
        AST::StringLiteral { handler: String::from(handler), data: String::from(data) }
    }
}
