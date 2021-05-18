use super::*;

impl Default for FunctionPrototype {
    fn default() -> Self {
        todo!()
    }
}

impl FunctionPrototype {
    /// The name of this function
    /// If it's a lambda function, it has no name
    pub fn get_name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    pub fn set_name<S: Into<String>>(&mut self, name: String) {
        self.name = Some(name);
    }
}
