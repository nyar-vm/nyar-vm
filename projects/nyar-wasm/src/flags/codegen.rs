use super::*;

impl<'a, 'i> IntoWasm<'a, wast::component::Type<'i>> for FlagType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::Type<'i> {
        wast::component::Type {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: None,
            exports: Default::default(),
            def: self.as_wast(),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::Type<'i>> for FlagType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::Type<'i> {
        wast::core::Type {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: None,
            def: self.as_wast(),
            parent: None,
            final_type: None,
        }
    }
}
impl<'a, 'i> IntoWasm<'a, wast::component::TypeDef<'i>> for FlagType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::TypeDef<'i> {
        wast::component::TypeDef::Defined(self.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::TypeDef<'i>> for FlagType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::TypeDef<'i> {
        todo!()
    }
}
impl<'a, 'i> IntoWasm<'a, ComponentDefinedType<'i>> for FlagType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentDefinedType<'i> {
        ComponentDefinedType::Enum(self.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, Enum<'i>> for FlagType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Enum<'i> {
        Enum { names: self.fields.iter().map(|(_, v)| v.name.as_ref()).collect() }
    }
}
