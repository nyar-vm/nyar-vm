use super::*;

/// - `Number`: raw number represent
#[derive(Clone)]
pub struct ByteLiteral {
    pub handler: char,
    pub value: String,
}

impl Debug for ByteLiteral {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "0{}{}", self.handler, self.value)
    }
}
