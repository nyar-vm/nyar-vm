use crate::{Identifier, IndexedIterator, NyarType, NyarValue};
use indexmap::IndexMap;

pub struct StructureBuilder {
    fields: IndexMap<String, FieldBuilder>,
}

pub struct FieldBuilder {
    name: Identifier,
    default: NyarValue,
}

impl StructureBuilder {
    pub fn fields(&self) -> IndexedIterator<FieldBuilder> {
        IndexedIterator::new(&self.fields)
    }
}

impl FieldBuilder {
    pub fn typing(&self) -> NyarType {
        self.default.as_type()
    }
}
