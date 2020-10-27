use super::*;

#[derive(Debug)]
pub enum NyarErrorKind {
    InvalidOperationInfix { op: String, lhs: String, rhs: String },
    InvalidOperationPrefix { op: String, item_type: String },
    InvalidOperationSuffix { op: String, item_type: String },
    InvalidCast { item_type: String },
    InvalidIndex { index: String, item_type: String },
    InvalidIterator { item_type: String },
    IfLost,
    IfNonBoolean,
    VariableNotFound { name: String },
    WriteUnwritable { name: String },
    CustomErrorText { text: String },
    LexerError { info: String },
    IOError(std::io::Error),
    FormatError(std::fmt::Error),
    ParseIntegerError(std::num::ParseIntError),
}

impl Display for NyarErrorKind {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            NyarErrorKind::IOError(e) => {
                write!(f, "{}", e)
            }
            NyarErrorKind::InvalidOperationInfix { op, lhs, rhs } => {
                write!(f, "InvalidOperation: Unable to apply operator `{}` on type `{}` and `{}`", op, lhs, rhs)
            }
            NyarErrorKind::InvalidOperationPrefix { op, item_type } => {
                write!(f, "InvalidOperation: Unable to apply prefix operator `{}` on type `{}`", op, item_type)
            }
            NyarErrorKind::InvalidOperationSuffix { op, item_type } => {
                write!(f, "InvalidOperation: Unable to apply suffix operator `{}` on type `{}`", op, item_type)
            }
            NyarErrorKind::InvalidIndex { index, item_type } => {
                write!(f, "IndexError: Unable to get index {} on type `{}`", index, item_type)
            }
            NyarErrorKind::InvalidIterator { item_type } => {
                write!(f, "IteratorError: Type `{}` is not an iterable element", item_type)
            }
            NyarErrorKind::IfLost => {
                write!(f, "IfError: If statements are not exhaustive")
            }
            NyarErrorKind::IfNonBoolean => {
                write!(f, "IfError: The conditional judgment is not a boolean value")
            }
            NyarErrorKind::FormatError(_) => {
                write!(f, "FormatError")
            }
            NyarErrorKind::LexerError { .. } => {
                write!(f, "LexerError")
            }
            NyarErrorKind::InvalidCast { item_type } => {
                write!(f, "CastError: Cast target can't be `{}`", item_type)
            }
            NyarErrorKind::VariableNotFound { name } => {
                write!(f, "MissingError: Cannot find variable `{}`", name)
            }
            NyarErrorKind::WriteUnwritable { name } => {
                write!(f, "WriteError: Attempt to write a non-writable item `{}`", name)
            }
            NyarErrorKind::CustomErrorText { text } => write!(f, "{}", text),
            _ => {
                unimplemented!("{:?}", self)
            }
        }
    }
}
