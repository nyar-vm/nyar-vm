use super::*;

#[derive(Default)]
pub struct ExternalSection {
    items: IndexMap<String, ExternalType>,
}

impl ExternalSection {
    pub fn insert(&mut self, item: ExternalType) -> Option<ExternalType> {
        self.items.insert(item.name().to_string(), item)
    }
}
impl<'i> IntoIterator for &'i ExternalSection {
    type Item = (usize, &'i str, &'i ExternalType);
    type IntoIter = IndexedIterator<'i, ExternalType>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}
