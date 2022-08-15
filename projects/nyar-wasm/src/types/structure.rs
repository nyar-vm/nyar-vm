use super::*;
use crate::{helpers::IndexedIterator, values::WasmValue};

#[derive(Debug)]
pub struct StructureType {
    pub symbol: WasmSymbol,
    pub fields: IndexMap<String, FieldType>,
    pub span: FileSpan,
}
#[derive(Debug)]
pub struct FieldType {
    pub name: WasmSymbol,
    pub mutable: bool,
    pub r#type: WasmType,
    pub default: WasmValue,
}

impl From<StructureType> for TypeItem {
    fn from(value: StructureType) -> Self {
        TypeItem::Structure(value)
    }
}

impl StructureType {
    pub fn new(name: WasmSymbol) -> Self {
        Self { symbol: name, fields: Default::default(), span: Default::default() }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn fields(&self) -> IndexedIterator<FieldType> {
        IndexedIterator::new(&self.fields)
    }
    pub fn add_field(&mut self, field: FieldType) {
        self.fields.insert(field.name.to_string(), field);
    }
    pub fn with_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = FieldType>,
    {
        for field in fields {
            self.add_field(field);
        }
        self
    }
}

impl FieldType {
    pub fn new(name: WasmSymbol) -> Self {
        Self { name, mutable: false, r#type: WasmType::Any { nullable: false }, default: WasmValue::Any }
    }
    pub fn with_type(self, r#type: WasmType) -> Self {
        Self { r#type, ..self }
    }
    pub fn with_default(self, default: WasmValue) -> Self {
        Self { default, ..self }
    }

    pub fn set_nullable(&mut self, nullable: bool) {
        self.r#type.set_nullable(nullable);
    }

    pub fn with_mutable(self) -> Self {
        Self { mutable: true, ..self }
    }
    pub fn with_readonly(self) -> Self {
        Self { mutable: false, ..self }
    }
    pub fn r#type(&self) -> WasmType {
        self.default.as_type()
    }
}

impl<'a, 'i> IntoWasm<'a, Type<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Type<'i> {
        Type {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            def: TypeDef::Struct(self.as_wast()),
            parent: None,
            final_type: Some(true),
        }
    }
}
impl<'a, 'i> IntoWasm<'a, StructType<'i>> for StructureType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StructType<'i> {
        StructType { fields: self.fields().map(|(_, _, field)| field.as_wast()).collect() }
    }
}

impl<'a, 'i> IntoWasm<'a, StructField<'i>> for FieldType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StructField<'i> {
        StructField { id: Id::type_id(self.name.as_ref()), mutable: self.mutable, ty: self.r#type.as_wast() }
    }
}
