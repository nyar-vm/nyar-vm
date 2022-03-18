use indexmap::{map::Values, IndexMap};
use nyar_error::NyarError;
use nyar_hir::{FunctionType, IndexedIterator};
use wasm_encoder::{Function, FunctionSection};

#[derive(Default)]
pub struct FunctionBuilder {
    items: IndexMap<String, FunctionItem>,
}
pub struct FunctionItem {
    pub name: String,
    pub export: bool,
    pub typing: FunctionType,
    pub body: Function,
}

impl<'i> IntoIterator for &'i FunctionBuilder {
    type Item = (usize, &'i str, &'i FunctionItem);
    type IntoIter = IndexedIterator<'i, FunctionItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
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
    pub fn build(&self) -> FunctionSection {
        let mut functions = FunctionSection::default();
        for i in 0..self.items.len() {
            functions.function(i as u32);
        }
        functions
    }
}