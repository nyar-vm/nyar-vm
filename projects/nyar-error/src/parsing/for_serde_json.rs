impl From<toml::de::Error> for SyntaxError {
    fn from(value: ParseBigIntError) -> Self {
        Self::new(value.to_string())
    }
}
