use super::*;
use wast::component::{CoreType, CoreTypeDef};

impl<'a, 'i> IntoWasm<'a, wast::component::Type<'i>> for StructureItem
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

impl<'a, 'i> IntoWasm<'a, wast::core::Type<'i>> for StructureType
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
impl<'a, 'i> IntoWasm<'a, CoreType<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreType<'i> {
        CoreType { span: Span::from_offset(0), id: self.symbol.as_id(), name: None, def: CoreTypeDef::Def(self.as_wast()) }
    }
}
impl<'a, 'i> IntoWasm<'a, wast::component::TypeDef<'i>> for StructureItem
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::TypeDef<'i> {
        wast::component::TypeDef::Defined(self.as_wast())
    }
}
impl<'a, 'i> IntoWasm<'a, wast::core::TypeDef<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::TypeDef<'i> {
        wast::core::TypeDef::Struct(self.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentDefinedType<'i>> for StructureItem
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentDefinedType<'i> {
        ComponentDefinedType::Record(self.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, StructType<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StructType<'i> {
        StructType { fields: self.fields.values().map(|field| field.as_wast()).collect() }
    }
}
impl<'a, 'i> IntoWasm<'a, Record<'i>> for StructureItem
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Record<'i> {
        Record { fields: self.fields.values().map(|v| v.as_wast()).collect() }
    }
}

impl<'a, 'i> IntoWasm<'a, StructField<'i>> for FieldType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StructField<'i> {
        StructField { id: self.name.as_id(), mutable: !self.readonly, ty: self.r#type.as_wast() }
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
