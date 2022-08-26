use super::*;
use wast::token::Span;

impl<'a, 'i> IntoWasm<'a, wast::component::Type<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::Type<'i> {
        wast::component::Type {
            span: Span::from_offset(0),
            id: None,
            name: None,
            exports: Default::default(),
            def: wast::component::TypeDef::Defined(self.as_wast()),
        }
    }
}
impl<'a, 'i> IntoWasm<'a, wast::core::Type<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::Type<'i> {
        match self {
            WasmType::Structure(s) => s.as_wast(),
            _ => {
                todo!()
            }
        }
    }
}
