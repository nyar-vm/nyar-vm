use crate::exports::nyar::pe_assembly::types::PeError;

impl From<std::io::Error> for PeError {
    fn from(value: std::io::Error) -> Self {
        PeError::IoError
    }
}
