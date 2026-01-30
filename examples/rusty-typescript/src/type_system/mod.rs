pub enum ScriptType {
    Atom(AtomType),
    Union(Vec<ScriptType>),
    Intersection(Vec<ScriptType>),
}

pub enum AtomType {
    Integer(i64),
    Float(f64),
}
