use super::*;

/// - `Number`: raw number represent
#[derive(Clone, Debug)]
pub struct ByteLiteral {
    pub handler: char,
    pub value: String,
}

impl Display for ByteLiteral {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "0{}{}", self.handler, self.value)
    }
}
