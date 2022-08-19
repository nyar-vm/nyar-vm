use super::*;
use crate::{
    helpers::{IntoWasm, WasmName},
    FieldType, StructureType, WasmType,
};
use wast::{
    component::{
        Component, ComponentDefinedType, ComponentField, ComponentKind, ComponentValType, PrimitiveValType, Record,
        RecordField, Type, TypeDef,
    },
    core::{Custom, HeapType, ModuleField, Producers, RefType, ValType},
    token::{Index, NameAnnotation, Span},
    Wat,
};

impl<'a, 'i> IntoWasm<'a, Record<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Record<'i> {
        Record { fields: vec![] }
    }
}
impl<'a, 'i> IntoWasm<'a, RecordField<'i>> for FieldType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> RecordField<'i> {
        RecordField { name: self.name.as_ref(), ty: self.r#type.as_wast() }
    }
}

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
            Self::Any { .. } => {
                todo!()
            }
            Self::Array { .. } => {
                todo!()
            }
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
            _ => unreachable!(),
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

impl ModuleBuilder {
    pub fn build_component(&self) -> Result<Wat, NyarError> {
        let mut coms = vec![];
        self.build_types(&mut coms);
        Ok(Wat::Component(Component {
            span: Span::from_offset(0),
            id: None,
            name: Some(NameAnnotation { name: "runtime" }),
            kind: ComponentKind::Text(coms),
        }))
    }

    fn build_types<'a, 'i>(&'a self, m: &mut Vec<ComponentField<'i>>)
    where
        'a: 'i,
    {
        for ty in self.types.into_iter() {
            m.push(ComponentField::Type(ty.as_wast()))
        }
    }

    fn build_producer(&self, m: &mut Vec<ComponentField>) {
        let item = Producers {
            fields: vec![
                ("language", vec![("valkyrie", "2024"), ("player", "berserker")]),
                ("processed-by", vec![("nyar-wasm", env!("CARGO_PKG_VERSION"))]),
            ],
        };
        m.push(ComponentField::Producers(item))
    }
}
