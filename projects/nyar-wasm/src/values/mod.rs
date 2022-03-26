use crate::helpers::{Id, WasmOutput};
use wast::{
    core::{Expression, Global, GlobalKind},
    token::{NameAnnotation, Span},
};

impl<'a, 'i> WasmOutput<'a, Global<'i>> for nyar_hir::NamedValue
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Global<'i> {
        let offset = self.span.get_start();
        Global {
            span: Span::from_offset(offset),
            id: Id::type_id(self.symbol.as_ref(), offset),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            exports: Default::default(),
            ty: wast::core::GlobalType { ty: self.value.as_wast(), mutable: self.mutable() },
            kind: GlobalKind::Inline(Expression { instrs: Box::new([]) }),
        }
    }
}
