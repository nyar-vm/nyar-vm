use indexmap::{map::Iter, IndexMap};

pub struct IndexedIterator<'i, T> {
    iter: Iter<'i, String, T>,
    index: usize,
}

impl<'i, T> Iterator for IndexedIterator<'i, T> {
    type Item = (usize, &'i str, &'i T);

    fn next(&mut self) -> Option<Self::Item> {
        let (name, item) = self.iter.next()?;
        let index = self.index;
        self.index += 1;
        Some((index, name, item))
    }
}
impl<'i, T> DoubleEndedIterator for IndexedIterator<'i, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let (name, item) = self.iter.next_back()?;
        let index = self.index;
        self.index += 1;
        Some((index, name, item))
    }
}

impl<'i, T> ExactSizeIterator for IndexedIterator<'i, T> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}
impl<'i, T> IndexedIterator<'i, T> {
    pub fn new(map: &'i IndexMap<String, T>) -> Self {
        Self { iter: map.iter(), index: 0 }
    }
}
