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
    LexerError { info: String },
    IOError(std::io::Error),
    FormatError(std::fmt::Error),
}


impl Display for NyarErrorKind {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            NyarErrorKind::IOError(e) => {
                write!(f, "{}", e)
            }
            NyarErrorKind::InvalidOperationInfix { op, lhs, rhs } => {
                writeln!(f, "InvalidOperation: Unable to apply operator `{}` on type `{}` and `{}`", op, lhs, rhs)?;
                write!(f, "--> {}:{}", position.start.line + 1, position.start.character + 1)
            }
            NyarErrorKind::InvalidOperationPrefix { op, item_type } => {
                writeln!(f, "InvalidOperation: Unable to apply prefix operator `{}` on type `{}`", op, item_type)?;

            }
            NyarErrorKind::InvalidOperationSuffix { op, item_type } => {
                writeln!(f, "InvalidOperation: Unable to apply suffix operator `{}` on type `{}`", op, item_type)?;
                write!(f, "--> {}:{}", position.start.line + 1, position.start.character + 1)
            }
            NyarErrorKind::InvalidIndex { index, item_type } => {
                writeln!(f, "IndexError: Unable to get index {} on type `{}`", index, item_type)?;
                write!(f, "--> {}:{}", position.start.line + 1, position.start.character + 1)
            }
            NyarErrorKind::InvalidIterator { item_type } => {
                writeln!(f, "IteratorError: Type `{}` is not an iterable element", item_type)?;
                write!(f, "--> {}:{}", position.start.line + 1, position.start.character + 1)
            }
            NyarErrorKind::IfLost => {
                writeln!(f, "IfError: If statements are not exhaustive")?;
                write!(f, "--> {}:{}", position.start.line + 1, position.start.character + 1)
            }
            NyarErrorKind::IfNonBoolean => {
                writeln!(f, "IfError: The conditional judgment is not a boolean value")?;
                write!(f, "--> {}:{}", position.start.line + 1, position.start.character + 1)
            }
            NyarErrorKind::FormatError(_) => {
                write!(f, "FormatError")
            }
            NyarErrorKind::LexerError { .. } => {
                write!(f, "LexerError")
            }
            NyarErrorKind::InvalidCast { item_type } => {
                writeln!(f, "CastError: Cast target can't be `{}`", item_type)?;
                write!(f, "--> {}:{}", position.start.line + 1, position.start.character + 1)
            }
        }
    }
}


