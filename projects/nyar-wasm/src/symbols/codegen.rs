use super::*;

impl<'a, 'i> IntoWasm<'a, wast::component::InlineExport<'i>> for WasmExportName
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::InlineExport<'i> {
        let names = match &self.inner {
            Some(s) => vec![ComponentExternName(s.as_ref())],
            None => vec![],
        };
        wast::component::InlineExport { names }
    }
}
impl<'a, 'i> IntoWasm<'a, wast::core::InlineExport<'i>> for WasmExportName
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::InlineExport<'i> {
        let names = match &self.inner {
            Some(s) => vec![s.as_ref()],
            None => vec![],
        };
        wast::core::InlineExport { names }
    }
}

impl WasmSymbol {
    pub(crate) fn as_index<'a, 'i>(&'a self) -> Index<'i>
    where
        'a: 'i,
    {
        Index::Id(WasmName::new(self.inner.as_ref()))
    }
    pub(crate) fn as_id<'a, 'i>(&'a self) -> Option<Id<'i>>
    where
        'a: 'i,
    {
        Some(WasmName::new(self.inner.as_ref()))
    }
}
