use indexmap::{map::Iter, IndexMap};
use nyar_error::NyarError;
use std::{
    fs::File,
    intrinsics::transmute,
    io::Write,
    path::{Path, PathBuf},
};
use wast::{core::Instruction, token::Span};

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

pub(crate) trait IntoWasm<'a, Item> {
    fn as_wast(&'a self) -> Item;
}

pub trait WasmInstruction {
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i;
}

#[allow(dead_code)]
pub(crate) struct WasmName<'a> {
    name: &'a str,
    gen: u32,
    span: Span,
}

impl<'a> WasmName<'a> {
    pub fn new(name: &'a str) -> wast::token::Id<'a> {
        unsafe {
            let s = WasmName { name, gen: 0, span: Span::from_offset(0) };
            transmute::<WasmName, wast::token::Id>(s)
        }
    }
    pub fn id(name: &'a str) -> Option<wast::token::Id<'a>> {
        Some(Self::new(name))
    }
}

pub(crate) fn write_wasm_bytes(path: &Path, buffer: Result<Vec<u8>, wast::Error>) -> Result<PathBuf, NyarError> {
    match buffer {
        Ok(o) => {
            let mut file = File::create(path)?;
            file.write_all(&o)?;
        }
        Err(e) => Err(NyarError::custom(e))?,
    }
    Ok(path.canonicalize()?)
}
