use indexmap::{map::Iter, IndexMap};
use std::intrinsics::transmute;
use wast::{
    core::Instruction,
    token::{Index, Span},
};

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
    pub fn with_index(self, index: usize) -> Self {
        Self { index, ..self }
    }
}

pub trait IntoWasm<'a, Item> {
    fn as_wast(&'a self) -> Item;
}

pub trait WasmInstruction {
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i;
}

pub struct Id<'a> {
    name: &'a str,
    gen: u32,
    span: Span,
}

impl<'a> Id<'a> {
    pub fn new(name: &'a str) -> wast::token::Id<'a> {
        unsafe {
            let s = Id { name, gen: 0, span: Span::from_offset(0) };
            transmute::<Id, wast::token::Id>(s)
        }
    }
    pub fn type_id(name: &'a str) -> Option<wast::token::Id<'a>> {
        Some(Self::new(name))
    }

    pub fn type_index(name: &'a str) -> Option<Index> {
        Some(Index::Id(Self::new(name)))
    }
}
