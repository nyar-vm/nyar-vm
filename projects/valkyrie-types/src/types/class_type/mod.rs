#[derive(Debug, Default)]
pub struct ValkyrieClassType {
    namepath: Vec<String>,
}

impl ValkyrieClassType {
    pub fn new(namepath: String) -> Self {
        Self { namepath: namepath.split('.').map(|s| s.to_string()).collect() }
    }
}
