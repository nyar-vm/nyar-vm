use super::*;
impl<'a, 'i> IntoWasm<'a, Func<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Func<'i> {
        let exports = match &self.export {
            Some(s) => InlineExport { names: vec![s.as_ref()] },
            None => InlineExport { names: vec![] },
        };
        Func {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            exports,
            kind: self.as_wast(),
            ty: TypeUse { index: None, inline: Some(self.as_wast()) },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::FunctionType<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FunctionType<'i> {
        let input = self.input.iter().map(|(_, ty)| ty.as_wast()).collect::<Vec<_>>();
        let result = self.output.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        wast::core::FunctionType { params: Box::from(input), results: Box::from(result) }
    }
}

impl<'a, 'i> IntoWasm<'a, (Option<wast::token::Id<'a>>, Option<NameAnnotation<'a>>, ValType<'a>)> for ParameterType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> (Option<wast::token::Id<'a>>, Option<NameAnnotation<'a>>, ValType<'a>) {
        (Id::type_id(self.name.as_ref()), None, self.type_hint.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, FuncKind<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> FuncKind<'i> {
        FuncKind::Inline { locals: Box::new([]), expression: self.body.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, Expression<'i>> for FunctionBody
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Expression<'i> {
        let mut instrs = Vec::with_capacity(16);
        for i in self.into_iter() {
            i.emit(&mut instrs);
        }
        Expression { instrs: Box::from(instrs) }
    }
}
