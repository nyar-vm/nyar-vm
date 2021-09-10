use valkyrie_errors::{FileID, ValkyrieError};

pub use self::tokens::{keywords::ValkyrieKeyword, operators::ValkyrieOperator};

mod parser;
mod tokens;
pub mod testing;

#[derive(Debug, Default)]
pub struct ValkyrieParser {
    file: FileID,
    errors: Vec<ValkyrieError>,
}
