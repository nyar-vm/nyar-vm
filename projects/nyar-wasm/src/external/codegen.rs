use super::*;

impl<'a, 'i> IntoWasm<'a, Import<'i>> for ExternalType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Import<'i> {
        Import {
            span: Span::from_offset(0),
            module: self.module.as_ref(),
            field: self.field.as_ref(),
            item: ItemSig {
                span: Span::from_offset(0),
                id: WasmName::id(self.name()),
                name: None,
                kind: ItemKind::Func(TypeUse { index: None, inline: Some(self.as_wast()) }),
            },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::FunctionType<'i>> for ExternalType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FunctionType<'i> {
        let input = self.input.iter().map(|ty| (None, None, ty.as_wast())).collect::<Vec<_>>();
        let result = self.output.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        wast::core::FunctionType { params: Box::from(input), results: Box::from(result) }
    }
}
