#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ErrorLevels {
    Allow,
    Warning,
    Deny,
}

impl From<bool> for ErrorLevels {
    fn from(o: bool) -> Self {
        match o {
            true => Self::Allow,
            false => Self::Deny,
        }
    }
}

impl From<Option<bool>> for ErrorLevels {
    fn from(o: Option<bool>) -> Self {
        match o {
            None => Self::Warning,
            Some(v) => Self::from(v),
        }
    }
}
