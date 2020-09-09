use super::*;

#[derive(Debug, Clone)]
pub struct NumberLiteral {
    pub handler: Option<String>,
    pub value: String,
}

impl Display for NumberLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)?;
        if let Some(s) = &self.handler {
            f.write_str(s)?
        }
        Ok(())
    }
}
