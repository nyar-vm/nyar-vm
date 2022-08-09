use super::*;
use crate::helpers::IndexedIterator;

#[derive(Default)]
pub struct ExternalSection {
    items: IndexMap<String, ExternalType>,
}

/// `@ffi("module", "field")`
pub struct ExternalType {
    pub module: WasmSymbol,
    pub field: WasmSymbol,
    pub input: Vec<WasmType>,
    pub output: Vec<WasmType>,
}
impl ExternalSection {
    pub fn insert(&mut self, item: ExternalType) -> Option<ExternalType> {
        self.items.insert(item.name(), item)
    }
}

impl ExternalType {
    pub fn new(module: &str, field: &str) -> ExternalType {
        Self { module: WasmSymbol::new(module), field: WasmSymbol::new(field), input: vec![], output: vec![] }
    }
    pub fn name(&self) -> String {
        format!("{}.{}", self.module, self.field)
    }
    pub fn with_input<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = WasmType>,
    {
        self.input.extend(inputs);
        self
    }
    pub fn with_output<I>(mut self, outputs: I) -> Self
    where
        I: IntoIterator<Item = WasmType>,
    {
        self.output.extend(outputs);
        self
    }
}

impl<'i> IntoIterator for &'i ExternalSection {
    type Item = (usize, &'i str, &'i ExternalType);
    type IntoIter = IndexedIterator<'i, ExternalType>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl<'a, 'i> WasmOutput<'a, Import<'i>> for ExternalType
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
                id: None,
                name: None,
                kind: ItemKind::Func(TypeUse { index: None, inline: Some(self.as_wast()) }),
            },
        }
    }
}

impl<'a, 'i> WasmOutput<'a, wast::core::FunctionType<'i>> for ExternalType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FunctionType<'i> {
        let input = self.input.iter().map(|ty| (None, None, ty.as_wast())).collect::<Vec<_>>();
        let result = self.output.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        wast::core::FunctionType { params: Box::from(input), results: Box::from(result) }
    }
}
