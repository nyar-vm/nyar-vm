use super::*;

#[derive(Default)]
pub struct FunctionSection {
    items: IndexMap<String, FunctionType>,
}

impl<'i> IntoIterator for &'i FunctionSection {
    type Item = (usize, &'i str, &'i FunctionType);
    type IntoIter = IndexedIterator<'i, FunctionType>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl FunctionSection {
    pub fn insert(&mut self, item: FunctionType) {
        self.items.insert(item.symbol.to_string(), item);
    }
}
