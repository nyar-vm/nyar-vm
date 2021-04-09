use super::*;
use crate::ast::KVPair;

impl VLanguage for Symbol {
    fn v_format<'a, 'b>(&'a self, arena: &'b Arena<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(self)
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for s in &self.scope {
            write_identifier(s, f)?;
            f.write_str("::")?;
        }
        write_identifier(&self.name, f)
    }
}

fn write_identifier(id: &str, f: &mut Formatter) -> std::fmt::Result {
    match is_valid_identifier(id) {
        true => write!(f, "{}", id),
        false => write!(f, "`{}`", id),
    }
}

fn is_valid_identifier(id: &str) -> bool {
    id.chars().all(|c| c.is_alphanumeric() || c == '_')
}

// impl Display for IntegerLiteral {
//     //noinspection DuplicatedCode
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         match self.handler.is_empty() {
//             true => write!(f, "{}", self.value),
//             false => write!(f, "{}_{}", self.value, self.handler),
//         }
//     }
// }

// impl Display for DecimalLiteral {
//     //noinspection DuplicatedCode
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         match self.handler.is_empty() {
//             true => write!(f, "{}", self.value),
//             false => write!(f, "{}_{}", self.value, self.handler),
//         }
//     }
// }
//
// impl Display for ByteLiteral {
//     fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
//         write!(f, "0{}{}", self.handler, self.value)
//     }
// }
//
// impl Display for KVPair {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}: {}", self.key, self.value)
//     }
// }
