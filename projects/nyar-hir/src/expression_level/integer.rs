use std::{num::ParseIntError, str::FromStr};

use valkyrie_errors::{ValkyrieError, ValkyrieResult};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValkyrieIntegerNode {
    pub hint: ValkyrieIdentifier,
    pub value: IBig,
}

impl Display for ValkyrieIntegerNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.hint.name)
    }
}

impl ValkyrieIntegerNode {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode {
            kind: ValkyrieASTKind::Integer(box self),
            span: FileSpan { file, head: range.start, tail: range.end },
        }
    }
}

impl ValkyrieASTNode {
    pub fn integer(num: &str, file: FileID, range: &Range<usize>, hint: Option<ValkyrieIdentifier>) -> ValkyrieResult<Self> {
        match IBig::from_str(num) {
            Ok(o) => Ok(ValkyrieIntegerNode { hint: hint.unwrap_or_default(), value: o }.to_node(file, range)),
            Err(_) => Err(ValkyrieError::syntax_error(format!("Invalid integer number: {}", num), FileSpan::new(file, range))),
        }
    }
    pub fn binary(num: &str, file: FileID, range: &Range<usize>) -> ValkyrieResult<Self> {
        match binary(num) {
            Ok(o) => Ok(o.to_node(file, range)),
            Err(_) => Err(ValkyrieError::syntax_error(format!("Invalid binary number: {}", num), FileSpan::new(file, range))),
        }
    }
    pub fn hex(num: &str, file: FileID, range: &Range<usize>) -> ValkyrieResult<Self> {
        match hex(num) {
            Ok(o) => Ok(o.to_node(file, range)),
            Err(_) => Err(ValkyrieError::syntax_error(format!("Invalid hex number: {}", num), FileSpan::new(file, range))),
        }
    }
}

pub fn binary(num: &str) -> Result<ValkyrieASTKind, ParseIntError> {
    assert!(num.starts_with("0b"));
    let num = &num[2..];
    let mut buffer = vec![];
    for byte in num.as_bytes().chunks(8) {
        let mut byte = byte.iter().map(|b| *b as char).collect::<String>();
        while byte.len() < 8 {
            byte.insert(0, '0');
        }
        let byte = u8::from_str_radix(&byte, 2)?;
        buffer.push(byte);
    }
    Ok(ValkyrieASTKind::Bytes(buffer))
}

pub fn hex(num: &str) -> Result<ValkyrieASTKind, ParseIntError> {
    assert!(num.starts_with("0x"));
    let num = &num[2..];
    let mut buffer = vec![];
    for byte in num.as_bytes().chunks(2) {
        let mut byte = byte.iter().map(|b| *b as char).collect::<String>();
        while byte.len() < 2 {
            byte.insert(0, '0');
        }
        let byte = u8::from_str_radix(&byte, 16)?;
        buffer.push(byte);
    }
    Ok(ValkyrieASTKind::Bytes(buffer))
}
