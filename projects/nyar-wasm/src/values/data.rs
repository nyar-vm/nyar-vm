use super::*;

#[derive(Default)]
pub struct DataSection {
    data: IndexMap<String, DataItem>,
}

impl<'i> IntoIterator for &'i DataSection {
    type Item = (usize, &'i str, &'i DataItem);
    type IntoIter = IndexedIterator<'i, DataItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.data)
    }
}
impl DataSection {
    pub fn insert(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item.symbol.to_string(), item)
    }
}
