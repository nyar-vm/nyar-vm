#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Level3 {
    Allow,
    Warning,
    Deny,
}

impl From<Option<bool>> for Level3 {
    fn from(o: Option<bool>) -> Self {
        match o {
            None => Self::Warning,
            Some(true) => Self::Allow,
            Some(false) => Self::Deny,
        }
    }
}
