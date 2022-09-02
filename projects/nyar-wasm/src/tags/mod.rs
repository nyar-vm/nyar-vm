use crate::{
    helpers::{IntoWasm, WasmName},
    ParameterType, WasmSymbol,
};
use indexmap::IndexMap;
use nyar_error::FileSpan;
use wast::{
    core::{Tag, TagKind, TagType, TypeUse},
    token::{NameAnnotation, Span},
};

pub struct ExceptionItem {
    pub symbol: WasmSymbol,
    pub data: IndexMap<String, ParameterType>,
    pub span: FileSpan,
}

impl<'a, 'i> IntoWasm<'a, Tag<'i>> for ExceptionItem
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::Tag<'i> {
        Tag {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            exports: Default::default(),
            ty: TagType::Exception(TypeUse { index: None, inline: None }),
            kind: TagKind::Inline(),
        }
    }
}
