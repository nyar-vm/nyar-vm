use indexmap::IndexMap;
use nyar_error::NyarError;
use wasm_encoder::{Function, FunctionSection};

#[derive(Default)]
pub struct FunctionBuilder {
    items: IndexMap<String, FunctionItem>,
}
pub struct FunctionItem {
    pub name: String,
    pub export: bool,
    pub typing: String,
    pub body: Function,
}

impl FunctionBuilder {
    pub fn get_id(&self, name: &str) -> Result<usize, NyarError> {
        match self.items.get_full(name) {
            Some((index, _, _)) => Ok(index),
            None => {
                panic!()
            }
        }
    }
    pub fn insert(&mut self, item: FunctionItem) {
        self.items.insert(item.name.clone(), item);
    }
    fn build_functions(&self) -> FunctionSection {
        let mut functions = FunctionSection::default();
        for i in 0..self.functions.len() {
            functions.function(i as u32);
        }
        functions
    }
}
