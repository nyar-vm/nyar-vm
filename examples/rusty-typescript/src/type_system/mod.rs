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

impl ScriptType {
    /// Check if a type is assignable to another
    pub fn is_assignable_to(&self, other: &ScriptType) -> bool {
        if self == other {
            return true;
        }

        match (self, other) {
            // Any can be assigned to anything, and anything can be assigned to Any
            (ScriptType::Atom(AtomType::Any), _) => true,
            (_, ScriptType::Atom(AtomType::Any)) => true,

            // Unknown can only be assigned to Any or Unknown
            (ScriptType::Atom(AtomType::Unknown), ScriptType::Atom(AtomType::Any)) => true,
            (ScriptType::Atom(AtomType::Unknown), ScriptType::Atom(AtomType::Unknown)) => true,
            (ScriptType::Atom(AtomType::Unknown), _) => false,

            // Never can be assigned to anything
            (ScriptType::Atom(AtomType::Never), _) => true,

            // Basic atom types
            (ScriptType::Atom(a), ScriptType::Atom(b)) => a == b,

            // Literal types can be assigned to their base types
            (ScriptType::Literal(LiteralType::String(_)), ScriptType::Atom(AtomType::String)) => true,
            (ScriptType::Literal(LiteralType::Number(_)), ScriptType::Atom(AtomType::Number)) => true,
            (ScriptType::Literal(LiteralType::Boolean(_)), ScriptType::Atom(AtomType::Boolean)) => true,
            (ScriptType::Literal(LiteralType::BigInt(_)), ScriptType::Atom(AtomType::BigInt)) => true,

            // Array covariance (simplified)
            (ScriptType::Array(inner_a), ScriptType::Array(inner_b)) => inner_a.is_assignable_to(inner_b),

            // Union types: T can be assigned to A | B if T is assignable to A or T is assignable to B
            (t, ScriptType::Union(variants)) => variants.iter().any(|v| t.is_assignable_to(v)),
            // A | B can be assigned to T if both A and B are assignable to T
            (ScriptType::Union(variants), t) => variants.iter().all(|v| v.is_assignable_to(t)),

            // Intersection types: T can be assigned to A & B if T is assignable to A and T is assignable to B
            (t, ScriptType::Intersection(variants)) => variants.iter().all(|v| t.is_assignable_to(v)),
            // A & B can be assigned to T if A is assignable to T or B is assignable to T
            (ScriptType::Intersection(variants), t) => variants.iter().any(|v| v.is_assignable_to(t)),

            // Null safety
            (ScriptType::Atom(AtomType::Null), _) => {
                // In strict null checks, this would be false unless the target is also Null or Any
                // For now, let's assume strict null checks are NOT always on for simplicity, 
                // or handle it via Union types like T | null
                false
            }
            (ScriptType::Atom(AtomType::Undefined), _) => false,

            // Object types (structural typing)
            (ScriptType::Object(obj_a), ScriptType::Object(obj_b)) => {
                for prop_b in &obj_b.properties {
                    let found = obj_a.properties.iter().find(|p| p.name == prop_b.name);
                    match found {
                        Some(prop_a) => {
                            if !prop_a.ty.is_assignable_to(&prop_b.ty) {
                                return false;
                            }
                        }
                        None => {
                            if !prop_b.optional {
                                return false;
                            }
                        }
                    }
                }
                true
            }

            _ => false,
        }
    }
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

impl TypeRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_builtins();
        registry
    }

    fn register_builtins(&mut self) {
        // Register built-in utility types as aliases or special markers
        // In a real implementation, these would be handled by the type checker
        // as they are often generic and mapped types.
        
        // Example: type Partial<T> = { [P in keyof T]?: T[P] };
        // Here we just ensure the registry is aware of them if needed.
    }

    pub fn get_type(&self, name: &str) -> Option<ScriptType> {
        if let Some(alias) = self.aliases.get(name) {
            return Some(alias.ty.clone());
        }
        // Basic resolution...
        None
    }
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
