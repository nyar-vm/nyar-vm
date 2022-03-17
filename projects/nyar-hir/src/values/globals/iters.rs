use super::*;
use crate::IndexedIterator;

impl<'i> IntoIterator for &'i GlobalBuilder {
    type Item = (usize, &'i str, &'i NamedValue);
    type IntoIter = IndexedIterator<'i, NamedValue>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}
