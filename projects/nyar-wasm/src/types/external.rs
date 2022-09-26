use super::*;
use crate::WasmExternalName;
use itertools::Itertools;

#[derive(Default)]
pub struct ImportSection {
    items: IndexMap<String, ImportFunction>,
}

impl ImportSection {
    pub fn insert(&mut self, item: ImportFunction) -> Option<ImportFunction> {
        self.items.insert(item.name().to_string(), item)
    }
}

impl<'i> IntoIterator for &'i ImportSection {
    type Item = (usize, &'i str, &'i ImportFunction);
    type IntoIter = IndexedIterator<'i, ImportFunction>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}
