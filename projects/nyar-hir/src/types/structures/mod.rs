use super::*;

pub struct StructureType {
    pub namepath: Identifier,
    pub fields: IndexMap<String, FieldType>,
}

pub struct FieldType {
    pub name: Symbol,
    pub mutable: bool,
    pub default: NyarValue,
}

impl From<StructureType> for TypeItem {
    fn from(value: StructureType) -> Self {
        TypeItem::Structure(value)
    }
}

impl StructureType {
    pub fn new(name: Identifier) -> Self {
        Self { namepath: name, fields: Default::default() }
    }
    pub fn name(&self) -> String {
        self.namepath.to_string()
    }
    pub fn fields(&self) -> IndexedIterator<FieldType> {
        IndexedIterator::new(&self.fields)
    }
    pub fn add_field(&mut self, field: FieldType) -> Option<FieldType> {
        self.fields.insert(field.name.to_string(), field)
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
    pub fn new(name: Symbol, default: NyarValue) -> Self {
        Self { name, mutable: false, default }
    }
    pub fn with_mutable(self) -> Self {
        Self { mutable: true, ..self }
    }
    pub fn with_readonly(self) -> Self {
        Self { mutable: false, ..self }
    }
    pub fn r#type(&self) -> NyarType {
        self.default.as_type()
    }
}
