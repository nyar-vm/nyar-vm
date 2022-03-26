use super::*;

impl<'a, 'i> WasmOutput<'a, Type<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Type<'i> {
        Type {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            def: TypeDef::Struct(self.as_wast()),
            parent: None,
            final_type: Some(true),
        }
    }
}
impl<'a, 'i> WasmOutput<'a, StructType<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StructType<'i> {
        StructType { fields: self.fields().map(|(_, _, field)| field.as_wast()).collect() }
    }
}

impl<'a, 'i> WasmOutput<'a, StructField<'i>> for FieldType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StructField<'i> {
        StructField { id: None, mutable: self.mutable, ty: self.default.as_wast() }
    }
}
