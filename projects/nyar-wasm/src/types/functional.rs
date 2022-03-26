use super::*;
use wast::core::{Expression, Func, FuncKind};

impl<'a, 'i> WasmOutput<'a, Func<'i>> for nyar_hir::FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Func<'i> {
        Func {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            exports: Default::default(),
            kind: self.as_wast(),
            ty: TypeUse { index: None, inline: None },
        }
    }
}
impl<'a, 'i> WasmOutput<'a, FuncKind<'i>> for nyar_hir::FunctionType {
    fn as_wast(&'a self) -> FuncKind<'i> {
        FuncKind::Inline { locals: Box::new([]), expression: Expression { instrs: Box::new([]) } }
    }
}
