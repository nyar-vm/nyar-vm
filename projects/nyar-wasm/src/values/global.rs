use super::*;

#[derive(Default)]
pub struct GlobalRegister {
    items: IndexMap<String, WasmVariable>,
}

impl GlobalRegister {
    pub fn insert(&mut self, item: WasmVariable) -> Option<WasmVariable> {
        self.items.insert(item.symbol.to_string(), item)
    }
}

impl<'i> IntoIterator for &'i GlobalRegister {
    type Item = (usize, &'i str, &'i WasmVariable);
    type IntoIter = IndexedIterator<'i, WasmVariable>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl<'a, 'i> WasmOutput<'a, Global<'i>> for WasmVariable
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Global<'i> {
        Global {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            exports: Default::default(),
            ty: wast::core::GlobalType { ty: self.value.as_wast(), mutable: self.mutable() },
            kind: GlobalKind::Inline(self.as_wast()),
        }
    }
}
