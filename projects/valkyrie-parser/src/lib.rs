use valkyrie_errors::{FileID, ValkyrieError};

pub use self::tokens::keywords::ValkyrieKeyword;

mod parser;
pub mod testing;
mod tokens;

#[derive(Debug, Default)]
pub struct ValkyrieParser {
    file: FileID,
    errors: Vec<ValkyrieError>,
}
