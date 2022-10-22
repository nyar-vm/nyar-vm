use super::*;

impl Display for ExternalFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.symbol)?;
        for (i, input) in self.inputs.iter().enumerate() {
            if i != 0 {
                f.write_str(", ")?
            }
            match input.name.as_ref().eq("self") {
                true => f.write_str("self")?,
                false => write!(f, "{}: {:#}", input.name, input.r#type.as_wasi_type())?,
            }
        }
        f.write_char(')')
    }
}
