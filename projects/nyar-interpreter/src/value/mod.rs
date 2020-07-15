mod from_native;

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Boolean(bool),
}
