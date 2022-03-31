use super::*;
use crate::{
    functions::{FunctionBody, FunctionType, ParameterType},
    helpers::IndexedIterator,
};

#[derive(Default)]
pub struct FunctionRegister {
    items: IndexMap<String, FunctionType>,
}
impl<'i> IntoIterator for &'i FunctionRegister {
    type Item = (usize, &'i str, &'i FunctionType);
    type IntoIter = IndexedIterator<'i, FunctionType>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl FunctionRegister {
    pub fn get_id(&self, name: &str) -> Result<usize, NyarError> {
        match self.items.get_full(name) {
            Some((index, _, _)) => return Ok(index),
            None => {}
        }
        Err(NyarError::custom(format!("missing function {name}")))
    }
    pub fn add_native(&mut self, item: FunctionType) {
        self.items.insert(item.symbol.to_string(), item);
    }
    pub fn get_natives(&self) -> IndexedIterator<FunctionType> {
        IndexedIterator::new(&self.items)
    }
}

impl<'a, 'i> WasmOutput<'a, Func<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Func<'i> {
        Func {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            exports: InlineExport { names: vec![self.symbol.as_ref()] },
            kind: self.as_wast(),
            ty: TypeUse { index: None, inline: Some(self.as_wast()) },
        }
    }
}

impl<'a, 'i> WasmOutput<'a, wast::core::FunctionType<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FunctionType<'i> {
        let input = self.input.iter().map(|(_, ty)| ty.as_wast()).collect::<Vec<_>>();
        let result = self.output.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        wast::core::FunctionType { params: Box::from(input), results: Box::from(result) }
    }
}

impl<'a, 'i> WasmOutput<'a, (Option<wast::token::Id<'a>>, Option<NameAnnotation<'a>>, ValType<'a>)> for ParameterType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> (Option<wast::token::Id<'a>>, Option<NameAnnotation<'a>>, ValType<'a>) {
        (Id::type_id(self.name.as_ref()), None, self.type_hint.as_wast())
    }
}

impl<'a, 'i> WasmOutput<'a, FuncKind<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> FuncKind<'i> {
        FuncKind::Inline { locals: Box::new([]), expression: self.body.as_wast() }
    }
}

impl<'a, 'i> WasmOutput<'a, Expression<'i>> for FunctionBody
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
