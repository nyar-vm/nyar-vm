use super::*;
use crate::{
    helpers::{IntoWasm, WasmName},
    WasmType,
};
use wast::{
    component::{
        Component, ComponentDefinedType, ComponentExternName, ComponentField, ComponentKind, ComponentValType, Func, FuncKind,
        InlineExport, PrimitiveValType, Type, TypeDef,
    },
    core::Producers,
    token::{NameAnnotation, Span},
    Wat,
};

impl<'a, 'i> IntoWasm<'a, ComponentValType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentValType<'i> {
        ComponentValType::Inline(self.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentDefinedType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentDefinedType<'i> {
        match self {
            Self::Structure(v) => ComponentDefinedType::Record(v.as_wast()),
            _ => ComponentDefinedType::Primitive(self.as_wast()),
        }
    }
}

impl<'a> IntoWasm<'a, PrimitiveValType> for WasmType {
    fn as_wast(&'a self) -> PrimitiveValType {
        match self {
            Self::Bool => PrimitiveValType::Bool,
            Self::U8 => PrimitiveValType::U8,
            Self::U16 => PrimitiveValType::U16,
            Self::U32 => PrimitiveValType::U32,
            Self::U64 => PrimitiveValType::U64,
            Self::I8 => PrimitiveValType::S8,
            Self::I16 => PrimitiveValType::S16,
            Self::I32 => PrimitiveValType::S32,
            Self::I64 => PrimitiveValType::S64,
            Self::F32 => PrimitiveValType::Float32,
            Self::F64 => PrimitiveValType::Float64,
            Self::Unicode => PrimitiveValType::Char,
            Self::UTF8Text => PrimitiveValType::String,
            _ => unreachable!("`{:?}` is not primitive type", self),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, Type<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Type<'i> {
        Type {
            span: Span::from_offset(0),
            id: None,
            name: None,
            exports: Default::default(),
            def: TypeDef::Defined(self.as_wast()),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, Option<NameAnnotation<'i>>> for ModuleBuilder
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Option<NameAnnotation<'i>> {
        if self.name.is_empty() { None } else { Some(NameAnnotation { name: self.name.as_str() }) }
    }
}

impl<'a, 'i> IntoWasm<'a, Producers<'i>> for ModuleBuilder
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Producers<'i> {
        Producers {
            fields: vec![
                ("language", vec![("valkyrie", "2024"), ("player", "berserker")]),
                ("processed-by", vec![("nyar-wasm", env!("CARGO_PKG_VERSION"))]),
            ],
        }
    }
}

impl<'a, 'i> IntoWasm<'a, Func<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Func<'i> {
        Func {
            span: Span::from_offset(0),
            id: WasmName::id(self.symbol.as_ref()),
            name: None,
            exports: self.as_wast(),
            kind: FuncKind::Lift { ty: Default::default(), info: Default::default() },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, InlineExport<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> InlineExport<'i> {
        match &self.export {
            Some(s) => InlineExport { names: vec![ComponentExternName(s.as_ref())] },
            None => InlineExport { names: vec![] },
        }
    }
}

impl ModuleBuilder {
    pub fn build_component(&self) -> Result<Wat, NyarError> {
        let mut coms = vec![];
        for ts in self.types.into_iter() {
            coms.push(ComponentField::Type(ts.as_wast()))
        }
        for fs in self.functions.values() {
            coms.push(ComponentField::Func(fs.as_wast()))
        }
        coms.push(ComponentField::Producers(self.as_wast()));
        Ok(Wat::Component(Component {
            span: Span::from_offset(0),
            id: None,
            name: self.as_wast(),
            kind: ComponentKind::Text(coms),
        }))
    }
}
