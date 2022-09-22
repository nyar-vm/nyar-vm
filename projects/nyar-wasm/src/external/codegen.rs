use super::*;

impl<'a, 'i> IntoWasm<'a, Import<'i>> for ExternalType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Import<'i> {
        Import {
            span: Span::from_offset(0),
            module: self.external.as_ref(),
            field: self.local.as_ref(),
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
        let input = self.inputs.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        let result = self.outputs.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        wast::core::FunctionType { params: Box::from(input), results: Box::from(result) }
    }
}
