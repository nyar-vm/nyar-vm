use crate::{ArrayType, Identifier, IndexedIterator, NyarValue, StructureType, Symbol};
use indexmap::IndexMap;

pub mod arrays;
pub mod structures;

pub enum TypeItem {
    Structure(StructureType),
    Array(ArrayType),
    // SubTyping { sub: SubType },
    // Recursion(RecursiveType),
}

#[derive(Default)]
pub struct TypeBuilder {
    items: IndexMap<String, TypeItem>,
}

impl<'i> IntoIterator for &'i TypeBuilder {
    type Item = (usize, &'i str, &'i TypeItem);
    type IntoIter = IndexedIterator<'i, TypeItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl TypeBuilder {
    pub fn insert(&mut self, item: TypeItem) -> Option<TypeItem> {
        self.items.insert(item.name().to_string(), item)
    }
}

impl TypeItem {
    pub fn name(&self) -> String {
        match self {
            TypeItem::Structure(v) => v.name(),
            TypeItem::Array(v) => v.namepath.to_string(),
        }
    }
}

pub enum NyarType {
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Any,
    Structure,
    Array,
}
