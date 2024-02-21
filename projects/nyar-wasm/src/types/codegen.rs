use super::*;
use crate::helpers::WasmName;
use wast::component::{Enum, List, Record, RecordField, Tuple};

impl<'a, 'i> IntoWasm<'a, wast::component::Type<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::Type<'i> {
        let id = match self {
            WasmType::U64 => None,
            WasmType::Structure(v) => WasmName::id(v.symbol.as_ref()),
            _ => {
                todo!("Cast `{:?}` to component type fail", self);
            }
        };

        wast::component::Type {
            span: Span::from_offset(0),
            id,
            name: None,
            exports: Default::default(),
            def: wast::component::TypeDef::Defined(self.as_wast()),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::Type<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::Type<'i> {
        match self {
            WasmType::Structure(s) => s.as_wast(),
            WasmType::Enumerate(s) => s.as_wast(),
            _ => unimplemented!("Cast `{:?}` to core type fail", self),
        }
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
            Self::Any { .. } => {
                todo!()
            }
            Self::Bool => ComponentDefinedType::Primitive(PrimitiveValType::Bool),
            Self::U8 => ComponentDefinedType::Primitive(PrimitiveValType::U8),
            Self::U16 => ComponentDefinedType::Primitive(PrimitiveValType::U16),
            Self::U32 => ComponentDefinedType::Primitive(PrimitiveValType::U32),
            Self::U64 => ComponentDefinedType::Primitive(PrimitiveValType::U64),
            Self::I8 => ComponentDefinedType::Primitive(PrimitiveValType::S8),
            Self::I16 => ComponentDefinedType::Primitive(PrimitiveValType::S16),
            Self::I32 => ComponentDefinedType::Primitive(PrimitiveValType::S32),
            Self::I64 => ComponentDefinedType::Primitive(PrimitiveValType::S64),
            Self::F32 => ComponentDefinedType::Primitive(PrimitiveValType::Float32),
            Self::F64 => ComponentDefinedType::Primitive(PrimitiveValType::Float64),
            Self::Unicode => ComponentDefinedType::Primitive(PrimitiveValType::Char),
            Self::UTF8Text => ComponentDefinedType::Primitive(PrimitiveValType::String),
            Self::Structure(v) => {
                let fields = v.fields.iter().map(|(k, v)| RecordField { name: k.as_ref(), ty: v.r#type.as_wast() }).collect();
                ComponentDefinedType::Record(Record { fields })
            }
            Self::Array(v) => ComponentDefinedType::List(List { element: Box::new(v.item_type.as_wast()) }),
            Self::Tuple(v) => ComponentDefinedType::Tuple(Tuple { fields: v.iter().map(|v| v.type_hint.as_wast()).collect() }),
            Self::Flag(_) => {
                todo!()
            }
            Self::Enumerate(v) => {
                ComponentDefinedType::Enum(Enum { names: v.fields.values().map(|v| v.name.as_ref()).collect() })
            }
            Self::Variant(_) => {
                todo!()
            }
            Self::Reference { .. } => {
                todo!()
            }
        }
    }
}

impl<'a, 'i> IntoWasm<'a, StorageType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StorageType<'i> {
        match self {
            WasmType::I8 => StorageType::I8,
            WasmType::I16 => StorageType::I16,
            _ => StorageType::Val(self.as_wast()),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, ValType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ValType<'i> {
        match self {
            Self::Bool => ValType::I32,
            Self::U8 => ValType::I32,
            Self::U16 => ValType::I32,
            Self::U32 => ValType::I32,
            Self::U64 => ValType::I64,
            Self::I8 => ValType::I32,
            Self::I16 => ValType::I32,
            Self::I32 => ValType::I32,
            Self::I64 => ValType::I64,
            Self::F32 => ValType::F32,
            Self::F64 => ValType::F64,
            Self::Unicode => ValType::I32,
            Self::Reference { name, nullable } => {
                ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Concrete(name.as_index()) })
            }
            Self::UTF8Text => {
                todo!()
            }
            Self::Flag(_) => {
                todo!()
            }
            Self::Enumerate(_) => {
                todo!()
            }
            Self::Variant(_) => {
                todo!()
            }
            Self::Tuple(_) => {
                todo!()
            }
            Self::Array(v) => ValType::Ref(RefType { nullable: v.nullable, heap: HeapType::Concrete(v.symbol.as_index()) }),
            Self::Structure(v) => ValType::Ref(RefType { nullable: v.nullable, heap: HeapType::Concrete(v.symbol.as_index()) }),
            Self::Any { nullable } => ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Any }),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, Vec<ValType<'i>>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Vec<ValType<'i>> {
        match self {
            Self::UTF8Text => {
                todo!()
            }
            Self::Any { .. } => {
                todo!()
            }
            Self::Flag(_) => {
                todo!()
            }
            Self::Enumerate(_) => {
                todo!()
            }
            Self::Variant(_) => {
                todo!()
            }
            Self::Reference { .. } => {
                todo!()
            }
            Self::Array(_) => {
                vec![ValType::I32]
            }
            Self::Tuple(v) => v.iter().map(|v| v.as_wast()).collect(),
            Self::Structure(t) => t.fields.iter().map(|v| v.1.r#type.as_wast()).collect(),
            primitive => vec![primitive.as_wast()],
        }
    }
}
