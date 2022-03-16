use super::*;
use nyar_hir::IndexedIterator;

impl<'i> IntoIterator for &'i GlobalBuilder {
    type Item = (usize, &'i str, &'i GlobalItem);
    type IntoIter = IndexedIterator<'i, GlobalItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}
