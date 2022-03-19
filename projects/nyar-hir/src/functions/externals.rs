use super::*;

impl FunctionExternalItem {
    pub fn name(&self) -> String {
        format!("{}.{}", self.module, self.field)
    }
}

impl FunctionExternalItem {
    pub fn new(module: &str, field: &str) -> FunctionExternalItem {
        Self { module: Symbol::new(module), field: Symbol::new(field), input: vec![], output: vec![] }
    }
    pub fn with_input<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = NyarType>,
    {
        self.input = inputs.into_iter().collect();
        self
    }
    pub fn with_output<I>(mut self, outputs: I) -> Self
    where
        I: IntoIterator<Item = NyarType>,
    {
        self.output = outputs.into_iter().collect();
        self
    }
}
