use super::*;
use crate::FunctionType;
use wast::component::{CoreFunc, CoreType, CoreTypeDef};

impl<'a, 'i> IntoWasm<'a, wast::component::Type<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::component::Type<'i> {
        wast::component::Type {
            span: Span::from_offset(0),
            id: None,
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
            _ => unimplemented!("Cast `{:?}` to core type fail", self),
        }
    }
}
impl<'a, 'i> IntoWasm<'a, CoreType<'i>> for WasmType
where
    'a: 'i,
{
    /// 怎么把组件函数降低为核心函数
    fn as_wast(&'a self) -> CoreType<'i> {
        match self {
            WasmType::Structure(v) => {
                CoreType { span: Span::from_offset(0), id: v.symbol.as_id(), name: None, def: CoreTypeDef::Def(self.as_wast()) }
            }
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
            Self::Structure(v) => ComponentDefinedType::Record(v.as_wast()),
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
            Self::Any { nullable } => ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Any }),
            Self::Structure(v) => ValType::Ref(RefType { nullable: false, heap: HeapType::Struct }),
            _ => unimplemented!("Cast `{:?}` to core value type fail", self),
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
