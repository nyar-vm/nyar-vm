use super::*;

#[derive(Default)]
pub struct ImportSection {
    items: IndexMap<String, ExternalFunctionType>,
}

impl ImportSection {
    pub fn insert(&mut self, item: ExternalFunctionType) -> Option<ExternalFunctionType> {
        self.items.insert(item.name().to_string(), item)
    }
}

impl<'i> IntoIterator for &'i ImportSection {
    type Item = (usize, &'i str, &'i ExternalFunctionType);
    type IntoIter = IndexedIterator<'i, ExternalFunctionType>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}
