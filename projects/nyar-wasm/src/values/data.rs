use super::*;

impl<'a, 'i> WasmOutput<'a, Data<'i>> for DataItem
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Data<'i> {
        Data {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            kind: DataKind::Active { memory: Index::Num(0, Span::from_offset(0)), offset: Expression { instrs: Box::new([]) } },
            data: vec![DataVal::String(self.data.as_slice())],
        }
    }
}
