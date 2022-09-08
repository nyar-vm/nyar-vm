use super::*;

impl<'a, 'i> IntoWasm<'a, Global<'i>> for WasmVariable
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Global<'i> {
        Global {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: None,
            exports: self.export.as_wast(),
            ty: GlobalType { ty: self.r#type.as_wast(), mutable: self.mutable },
            kind: GlobalKind::Inline(self.as_wast()),
        }
    }
}
