use super::*;

impl<'a, 'i> WasmOutput<'a, Type<'i>> for StructureType {
    fn as_wast(&'a self) -> Type<'i> {
        let offset = self.namepath.span.get_start();
        Type {
            span: Span::from_offset(offset),
            id: Id::type_id("point", offset),
            name: Some(NameAnnotation { name: "Point" }),
            def: TypeDef::Struct(self.as_wast()),
            parent: None,
            final_type: Some(true),
        }
    }
}
impl<'a, 'i> WasmOutput<'a, StructType<'i>> for StructureType {
    fn as_wast(&'a self) -> StructType<'i> {
        StructType { fields: self.fields().map(|(_, _, field)| field.as_wast()).collect() }
    }
}

impl<'a, 'i> WasmOutput<'a, StructField<'i>> for FieldType {
    fn as_wast(&'a self) -> StructField<'i> {
        StructField { id: None, mutable: self.mutable, ty: StorageType::I8 }
    }
}
