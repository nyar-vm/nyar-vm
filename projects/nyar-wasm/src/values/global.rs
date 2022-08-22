use super::*;
use crate::helpers::IndexedIterator;
use wast::core::{GlobalType, InlineExport};

#[derive(Default)]
pub struct GlobalSection {
    items: IndexMap<String, WasmVariable>,
}

impl GlobalSection {
    pub fn insert(&mut self, item: WasmVariable) -> Option<WasmVariable> {
        self.items.insert(item.symbol.to_string(), item)
    }
}

impl<'i> IntoIterator for &'i GlobalSection {
    type Item = (usize, &'i str, &'i WasmVariable);
    type IntoIter = IndexedIterator<'i, WasmVariable>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl<'a, 'i> IntoWasm<'a, Global<'i>> for WasmVariable
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Global<'i> {
        Global {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: None,
            exports: InlineExport { names: vec![self.symbol.as_ref()] },
            ty: GlobalType { ty: self.r#type.as_wast(), mutable: self.mutable },
            kind: GlobalKind::Inline(self.as_wast()),
        }
    }
}
