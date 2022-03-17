use crate::{Identifier, IndexedIterator, NyarType, NyarValue};
use indexmap::IndexMap;

pub struct StructureType {
    fields: IndexMap<String, FieldBuilder>,
}

pub struct FieldBuilder {
    name: Identifier,
    default: NyarValue,
}

impl StructureType {
    pub fn fields(&self) -> IndexedIterator<FieldBuilder> {
        IndexedIterator::new(&self.fields)
    }
    pub fn insert_field(&mut self, field: FieldBuilder) -> Option<FieldBuilder> {
        self.fields.insert(field.name.to_string(), field)
    }
}

impl FieldBuilder {
    pub fn new(name: Identifier, default: NyarValue) -> Self {
        Self { name, default }
    }
    pub fn r#type(&self) -> NyarType {
        self.default.as_type()
    }
}
