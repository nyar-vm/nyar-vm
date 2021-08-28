use super::*;

#[derive(Debug)]
pub struct InternalError {
    /// The message to display to the user
    pub message: String,
}

impl Error for InternalError {}

impl Display for InternalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to parse because")
    }
}

impl Diagnostic for InternalError {}

impl From<()> for NyarError {
    fn from(_: ()) -> Self {
        NyarError::InternalError(box InternalError { message: "Internal error".to_string() })
    }
}
