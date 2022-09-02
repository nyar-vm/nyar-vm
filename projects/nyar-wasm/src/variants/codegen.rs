use super::*;
use crate::StructureType;
use wast::component::{Variant, VariantCase};

impl<'a, 'i> IntoWasm<'a, wast::component::Type<'i>> for VariantType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::Type<'i> {
        wast::component::Type {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            exports: Default::default(),
            def: self.as_wast(),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::Type<'i>> for VariantType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::Type<'i> {
        wast::core::Type {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            def: self.as_wast(),
            parent: None,
            final_type: None,
        }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::component::TypeDef<'i>> for VariantType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::TypeDef<'i> {
        wast::component::TypeDef::Defined(self.as_wast())
    }
}
impl<'a, 'i> IntoWasm<'a, wast::core::TypeDef<'i>> for VariantType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::TypeDef<'i> {
        // wast::core::TypeDef::Struct(self.as_wast())
        unimplemented!()
    }
}
impl<'a, 'i> IntoWasm<'a, ComponentDefinedType<'i>> for VariantType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentDefinedType<'i> {
        ComponentDefinedType::Variant(self.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, Variant<'i>> for VariantType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Variant<'i> {
        Variant { cases: self.fields.values().map(|s| s.as_wast()).collect() }
    }
}

impl<'a, 'i> IntoWasm<'a, VariantCase<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> VariantCase<'i> {
        VariantCase { span: Span::from_offset(0), id: self.symbol.as_id(), name: "", ty: None, refines: None }
    }
}
