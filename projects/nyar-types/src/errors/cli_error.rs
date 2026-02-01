#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    NoChunk,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::NoChunk => write!(f, "no chunk to execute"),
        }
    }
}

impl std::error::Error for CliError {}
