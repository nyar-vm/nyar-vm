use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptType {
    /// Basic types like number, string, boolean, etc.
    Atom(AtomType),
    /// Literal types like "hello", 42, true
    Literal(LiteralType),
    /// Array type: T[]
    Array(Box<ScriptType>),
    /// Tuple type: [T, U]
    Tuple(Vec<ScriptType>),
    /// Object type: { a: T, b?: U }
    Object(ObjectType),
    /// Function type: (a: T) => U
    Function(FunctionType),
    /// Union type: T | U
    Union(Vec<ScriptType>),
    /// Intersection type: T & U
    Intersection(Vec<ScriptType>),
    /// Named type reference: MyInterface<T>
    Reference(TypeReference),
    /// Advanced type operators
    Operator(TypeOperator),
    /// Conditional type: T extends U ? X : Y
    Conditional(ConditionalType),
    /// Mapped type: { [K in keyof T]: U }
    Mapped(MappedType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomType {
    Any,
    Unknown,
    Never,
    Void,
    Null,
    Undefined,
    Number,
    BigInt,
    String,
    Boolean,
    Symbol,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralType {
    String(String),
    Number(f64),
    Boolean(bool),
    BigInt(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectType {
    pub properties: Vec<Property>,
    pub index_signatures: Vec<IndexSignature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub ty: ScriptType,
    pub optional: bool,
    pub readonly: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexSignature {
    pub key_name: String,
    pub key_type: AtomType, // usually String or Number
    pub value_type: ScriptType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    pub params: Vec<Parameter>,
    pub return_type: Box<ScriptType>,
    pub type_params: Vec<TypeParameter>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: ScriptType,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeParameter {
    pub name: String,
    pub constraint: Option<Box<ScriptType>>,
    pub default: Option<Box<ScriptType>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeReference {
    pub name: String,
    pub args: Vec<ScriptType>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeOperator {
    KeyOf(Box<ScriptType>),
    TypeOf(String),
    ReadOnly(Box<ScriptType>),
    Partial(Box<ScriptType>),
    Required(Box<ScriptType>),
    Pick(Box<ScriptType>, Vec<String>),
    Omit(Box<ScriptType>, Vec<String>),
    Record(Box<ScriptType>, Box<ScriptType>),
    Infer(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalType {
    pub check_type: Box<ScriptType>,
    pub extends_type: Box<ScriptType>,
    pub true_type: Box<ScriptType>,
    pub false_type: Box<ScriptType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MappedType {
    pub key_name: String,
    pub key_type: Box<ScriptType>,
    pub value_type: Box<ScriptType>,
    pub readonly: Option<bool>,
    pub optional: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    pub interfaces: HashMap<String, InterfaceDefinition>,
    pub aliases: HashMap<String, AliasDefinition>,
    pub enums: HashMap<String, EnumDefinition>,
}

#[derive(Debug, Clone)]
pub struct InterfaceDefinition {
    pub name: String,
    pub type_params: Vec<TypeParameter>,
    pub extends: Vec<TypeReference>,
    pub body: ObjectType,
}

#[derive(Debug, Clone)]
pub struct AliasDefinition {
    pub name: String,
    pub type_params: Vec<TypeParameter>,
    pub ty: ScriptType,
}

#[derive(Debug, Clone)]
pub struct EnumDefinition {
    pub name: String,
    pub members: Vec<EnumMember>,
}

#[derive(Debug, Clone)]
pub struct EnumMember {
    pub name: String,
    pub value: Option<LiteralType>,
}
