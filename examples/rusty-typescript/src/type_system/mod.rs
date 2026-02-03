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
    /// Template literal type: `head${T}tail`
    TemplateLiteral(Vec<TemplateElement>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateElement {
    String(String),
    Type(Box<ScriptType>),
}

impl ScriptType {
    /// Resolve the type to its concrete representation, expanding aliases and applying operators
    pub fn resolve(&self, registry: &TypeRegistry) -> ScriptType {
        match self {
            ScriptType::Reference(ref_ty) => {
                if let Some(mut resolved) = registry.get_type(&ref_ty.name) {
                    // Handle generics if any
                    // This is a simplified version; real generics would involve substitution
                    resolved
                } else {
                    self.clone()
                }
            }
            ScriptType::Operator(op) => match op {
                TypeOperator::Partial(inner) => {
                    let resolved = inner.resolve(registry);
                    if let ScriptType::Object(mut obj) = resolved {
                        for prop in &mut obj.properties {
                            prop.optional = true;
                        }
                        ScriptType::Object(obj)
                    } else {
                        resolved
                    }
                }
                TypeOperator::Required(inner) => {
                    let resolved = inner.resolve(registry);
                    if let ScriptType::Object(mut obj) = resolved {
                        for prop in &mut obj.properties {
                            prop.optional = false;
                        }
                        ScriptType::Object(obj)
                    } else {
                        resolved
                    }
                }
                TypeOperator::ReadOnly(inner) => {
                    let resolved = inner.resolve(registry);
                    if let ScriptType::Object(mut obj) = resolved {
                        for prop in &mut obj.properties {
                            prop.readonly = true;
                        }
                        ScriptType::Object(obj)
                    } else {
                        resolved
                    }
                }
                TypeOperator::Pick(inner, keys) => {
                    let resolved = inner.resolve(registry);
                    if let ScriptType::Object(mut obj) = resolved {
                        obj.properties.retain(|p| keys.contains(&p.name));
                        ScriptType::Object(obj)
                    } else {
                        resolved
                    }
                }
                TypeOperator::Omit(inner, keys) => {
                    let resolved = inner.resolve(registry);
                    if let ScriptType::Object(mut obj) = resolved {
                        obj.properties.retain(|p| !keys.contains(&p.name));
                        ScriptType::Object(obj)
                    } else {
                        resolved
                    }
                }
                TypeOperator::Record(key_type, value_type) => {
                    // Record<K, V> is basically { [P in K]: V }
                    // Simplified: return an object with an index signature
                    let mut obj = ObjectType {
                        properties: Vec::new(),
                        index_signatures: Vec::new(),
                    };
                    let key_atom = match key_type.resolve(registry) {
                        ScriptType::Atom(a) => a,
                        _ => AtomType::String,
                    };
                    obj.index_signatures.push(IndexSignature {
                        key_name: "key".to_string(),
                        key_type: key_atom,
                        value_type: *value_type.clone(),
                    });
                    ScriptType::Object(obj)
                }
                _ => self.clone(),
            },
            _ => self.clone(),
        }
    }

    /// Check if a type is assignable to another
    pub fn is_assignable_to(&self, other: &ScriptType, registry: &TypeRegistry) -> bool {
        if self == other {
            return true;
        }

        let s = self.resolve(registry);
        let o = other.resolve(registry);

        match (&s, &o) {
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

            // Null safety (strict mode simulation)
            (ScriptType::Atom(AtomType::Null), ScriptType::Atom(AtomType::Null)) => true,
            (ScriptType::Atom(AtomType::Undefined), ScriptType::Atom(AtomType::Undefined)) => true,
            (ScriptType::Atom(AtomType::Null), _) => false,
            (ScriptType::Atom(AtomType::Undefined), _) => false,

            // Literal types can be assigned to their base types
            (ScriptType::Literal(LiteralType::String(_)), ScriptType::Atom(AtomType::String)) => true,
            (ScriptType::Literal(LiteralType::Number(_)), ScriptType::Atom(AtomType::Number)) => true,
            (ScriptType::Literal(LiteralType::Boolean(_)), ScriptType::Atom(AtomType::Boolean)) => true,
            (ScriptType::Literal(LiteralType::BigInt(_)), ScriptType::Atom(AtomType::BigInt)) => true,
            (ScriptType::Literal(a), ScriptType::Literal(b)) => a == b,

            // Array covariance (simplified)
            (ScriptType::Array(inner_a), ScriptType::Array(inner_b)) => {
                inner_a.is_assignable_to(inner_b, registry)
            }

            // Tuple types
            (ScriptType::Tuple(elems_a), ScriptType::Tuple(elems_b)) => {
                if elems_a.len() != elems_b.len() {
                    return false;
                }
                for (a, b) in elems_a.iter().zip(elems_b.iter()) {
                    if !a.is_assignable_to(b, registry) {
                        return false;
                    }
                }
                true
            }

            // Function types (contravariant parameters, covariant return type)
            (ScriptType::Function(func_a), ScriptType::Function(func_b)) => {
                // Return type covariance
                if !func_a.return_type.is_assignable_to(&func_b.return_type, registry) {
                    return false;
                }
                // Parameters contravariance
                // TypeScript allows assigning a function with fewer parameters to one with more
                if func_a.params.len() > func_b.params.len() {
                    return false;
                }
                for (p_a, p_b) in func_a.params.iter().zip(func_b.params.iter()) {
                    // Contravariance: p_b.ty must be assignable to p_a.ty
                    if !p_b.ty.is_assignable_to(&p_a.ty, registry) {
                        return false;
                    }
                }
                true
            }

            // Union types: T can be assigned to A | B if T is assignable to A or T is assignable to B
            (t, ScriptType::Union(variants)) => {
                variants.iter().any(|v| t.is_assignable_to(v, registry))
            }
            // A | B can be assigned to T if both A and B are assignable to T
            (ScriptType::Union(variants), t) => {
                variants.iter().all(|v| v.is_assignable_to(t, registry))
            }

            // Intersection types: T can be assigned to A & B if T is assignable to A and T is assignable to B
            (t, ScriptType::Intersection(variants)) => {
                variants.iter().all(|v| t.is_assignable_to(v, registry))
            }
            // A & B can be assigned to T if A is assignable to T or B is assignable to T
            (ScriptType::Intersection(variants), t) => {
                variants.iter().any(|v| v.is_assignable_to(t, registry))
            }

            // Object types (structural typing)
            (ScriptType::Object(obj_a), ScriptType::Object(obj_b)) => {
                for prop_b in &obj_b.properties {
                    let found = obj_a.properties.iter().find(|p| p.name == prop_b.name);
                    match found {
                        Some(prop_a) => {
                            if !prop_a.ty.is_assignable_to(&prop_b.ty, registry) {
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

    pub fn register_interface(&mut self, def: InterfaceDefinition) {
        self.interfaces.insert(def.name.clone(), def);
    }

    pub fn register_alias(&mut self, def: AliasDefinition) {
        self.aliases.insert(def.name.clone(), def);
    }

    pub fn register_enum(&mut self, def: EnumDefinition) {
        self.enums.insert(def.name.clone(), def);
    }

    pub fn get_type(&self, name: &str) -> Option<ScriptType> {
        if let Some(alias) = self.aliases.get(name) {
            return Some(alias.ty.clone());
        }
        if let Some(interface) = self.interfaces.get(name) {
            return Some(ScriptType::Object(interface.body.clone()));
        }
        if let Some(enum_def) = self.enums.get(name) {
            // Enum is basically a union of its literal values
            let variants = enum_def
                .members
                .iter()
                .filter_map(|m| m.value.as_ref().map(|v| ScriptType::Literal(v.clone())))
                .collect::<Vec<_>>();
            if variants.is_empty() {
                return Some(ScriptType::Atom(AtomType::Number));
            }
            return Some(ScriptType::Union(variants));
        }
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
