use super::*;
use crate::FlagType;
use wast::component::{ComponentDefinedType, List};

impl<'a, 'i> IntoWasm<'a, Type<'i>> for ArrayType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Type<'i> {
        Type {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            def: TypeDef::Array(self.as_wast()),
            parent: None,
            final_type: Some(true),
        }
    }
}
impl<'a, 'i> IntoWasm<'a, wast::core::ArrayType<'i>> for ArrayType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::ArrayType<'i> {
        wast::core::ArrayType { mutable: self.mutable, ty: self.item_type.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentDefinedType<'i>> for ArrayType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentDefinedType<'i> {
        ComponentDefinedType::List(self.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, List<'i>> for ArrayType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> List<'i> {
        List { element: Box::new(self.item_type.as_wast()) }
    }
}
