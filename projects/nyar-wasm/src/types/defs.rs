use super::*;

impl<'a, 'i> WasmOutput<'a, Type<'i>> for nyar_hir::ArrayType {
    fn as_wast(&'a self) -> Type<'i> {
        let offset = self.namepath.span.get_start();
        Type {
            span: Span::from_offset(offset),
            id: Id::type_id("point", offset),
            name: Some(NameAnnotation { name: "Point" }),
            def: TypeDef::Array(self.as_wast()),
            parent: None,
            final_type: Some(true),
        }
    }
}
impl<'a, 'i> WasmOutput<'a, ArrayType<'i>> for nyar_hir::ArrayType {
    fn as_wast(&'a self) -> ArrayType<'i> {
        ArrayType { mutable: self.mutable, ty: StorageType::I8 }
    }
}
