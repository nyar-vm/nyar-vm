use valkyrie_errors::{FileID, ValkyrieError};

mod parser;

#[derive(Debug, Default)]
pub struct ValkyrieParser {
    file: FileID,
    errors: Vec<ValkyrieError>,
}
