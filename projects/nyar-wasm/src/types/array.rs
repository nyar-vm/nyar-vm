use super::*;

impl<'a, 'i> WasmOutput<'a, Type<'i>> for nyar_hir::ArrayType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Type<'i> {
        Type {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            def: TypeDef::Array(self.as_wast()),
            parent: None,
            final_type: Some(true),
        }
    }
}
impl<'a, 'i> WasmOutput<'a, ArrayType<'i>> for nyar_hir::ArrayType {
    fn as_wast(&'a self) -> ArrayType<'i> {
        ArrayType { mutable: self.mutable, ty: self.item_type.as_wast() }
    }
}
