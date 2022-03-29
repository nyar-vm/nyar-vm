use super::*;

impl From<ArrayType> for TypeItem {
    fn from(value: ArrayType) -> Self {
        Self::Array(value)
    }
}
pub struct ArrayType {
    pub symbol: Symbol,
    pub mutable: bool,
    /// Item type of the array
    pub item_type: NyarType,
    pub span: FileSpan,
}

impl ArrayType {
    pub fn new(name: Symbol, item: NyarType) -> Self {
        Self { symbol: name, mutable: false, item_type: item, span: Default::default() }
    }
}

impl<'a, 'i> WasmOutput<'a, Type<'i>> for ArrayType
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
impl<'a, 'i> WasmOutput<'a, wast::core::ArrayType<'i>> for ArrayType {
    fn as_wast(&'a self) -> wast::core::ArrayType<'i> {
        wast::core::ArrayType { mutable: self.mutable, ty: self.item_type.as_wast() }
    }
}
