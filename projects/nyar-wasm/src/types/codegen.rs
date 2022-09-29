use super::*;
use crate::helpers::WasmName;
use wast::component::{List, Record, RecordField};

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

impl<'a, 'i> IntoWasm<'a, CoreType<'i>> for WasmType
where
    'a: 'i,
{
    /// 怎么把组件类型降低为核心类型
    fn as_wast(&'a self) -> CoreType<'i> {
        match self {
            WasmType::Structure(v) => v.as_wast(),
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
            Self::Structure(v) => {
                let fields = v.fields.iter().map(|(k, v)| RecordField { name: k.as_ref(), ty: v.r#type.as_wast() }).collect();
                ComponentDefinedType::Record(Record { fields })
            }
            Self::Array(v) => ComponentDefinedType::List(List { element: Box::new(v.item_type.as_wast()) }),
            _ => ComponentDefinedType::Primitive(self.as_wast()),
        }
    }
}

impl<'a> IntoWasm<'a, PrimitiveValType> for WasmType {
    fn as_wast(&'a self) -> PrimitiveValType {
        match self {
            Self::Bool => PrimitiveValType::Bool,
            Self::U8 => PrimitiveValType::U8,
            Self::U16 => PrimitiveValType::U16,
            Self::U32 => PrimitiveValType::U32,
            Self::U64 => PrimitiveValType::U64,
            Self::I8 => PrimitiveValType::S8,
            Self::I16 => PrimitiveValType::S16,
            Self::I32 => PrimitiveValType::S32,
            Self::I64 => PrimitiveValType::S64,
            Self::F32 => PrimitiveValType::Float32,
            Self::F64 => PrimitiveValType::Float64,
            Self::Unicode => PrimitiveValType::Char,
            Self::UTF8Text => PrimitiveValType::String,
            _ => unreachable!("`{:?}` is not primitive type", self),
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
            Self::Array(v) => ValType::Ref(RefType { nullable: v.nullable, heap: HeapType::Concrete(v.symbol.as_index()) }),
            Self::Structure(v) => ValType::Ref(RefType { nullable: v.nullable, heap: HeapType::Concrete(v.symbol.as_index()) }),
            Self::Any { nullable } => ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Any }),
            _ => unimplemented!("Cast `{:?}` to core value type fail", self),
        }
    }
}
