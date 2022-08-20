use super::*;

impl<'a, 'i> IntoWasm<'a, Type<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Type<'i> {
        Type {
            span: Span::from_offset(0),
            id: WasmName::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),

            exports: Default::default(),
            def: (),
        }
    }
}
impl<'a, 'i> IntoWasm<'a, StructType<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StructType<'i> {
        StructType { fields: self.fields().map(|(_, _, field)| field.as_wast()).collect() }
    }
}

impl<'a, 'i> IntoWasm<'a, StructField<'i>> for FieldType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StructField<'i> {
        StructField { id: WasmName::type_id(self.name.as_ref()), mutable: self.mutable, ty: self.r#type.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, Record<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Record<'i> {
        Record { fields: vec![] }
    }
}
impl<'a, 'i> IntoWasm<'a, RecordField<'i>> for FieldType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> RecordField<'i> {
        RecordField { name: self.name.as_ref(), ty: self.r#type.as_wast() }
    }
}
