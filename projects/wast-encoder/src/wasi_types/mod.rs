use std::{
    fmt::{Debug, Display, Formatter, Write},
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{
    dag::DependentGraph, DependenciesTrace, ExternalFunction, Identifier, VariantType, WasiModule, WasiParameter, WasiResource,
    WastEncoder,
};

mod display;

pub enum WasiValue {
    Integer8(i8),
    Integer16(i16),
    Integer32(i32),
    Integer64(i64),
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
    Float32(f32),
    Float64(f64),
}

impl Default for WasiValue {
    fn default() -> Self {
        todo!()
    }
}

impl WasiValue {
    pub fn create<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Integer8(v) => write!(w, "(i32.const {})", v)?,
            Self::Integer16(v) => write!(w, "(i32.const {})", v)?,
            Self::Integer32(v) => write!(w, "(i32.const {})", v)?,
            Self::Integer64(v) => write!(w, "(i64.const {})", v)?,
            Self::Unsigned8(v) => write!(w, "(i32.const {})", v)?,
            Self::Unsigned16(v) => write!(w, "(i32.const {})", v)?,
            Self::Unsigned32(v) => write!(w, "(i32.const {})", v)?,
            Self::Unsigned64(v) => write!(w, "(i64.const {})", v)?,
            Self::Float32(v) => write!(w, "(f32.const {})", v)?,
            Self::Float64(v) => write!(w, "(f64.const {})", v)?,
        }
        Ok(())
    }
    pub fn create_default<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        Self::default().create(w)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasiType {
    Integer8 {
        signed: bool,
    },
    Integer16 {
        signed: bool,
    },
    Integer32 {
        signed: bool,
    },
    Integer64 {
        signed: bool,
    },
    Option {
        inner: Box<WasiType>,
    },
    Result {
        success: Option<Box<WasiType>>,
        failure: Option<Box<WasiType>>,
    },
    Resource(WasiResource),
    Variant(VariantType),
    TypeHandler {
        /// Resource language name
        name: Identifier,
        own: bool,
    },
    Array {
        inner: Box<WasiType>,
    },
    /// A referenced type, the real type needs to be found later
    TypeAlias {
        /// Type language name
        name: Identifier,
    },
    External(Box<ExternalFunction>),
}

impl WasiType {
    pub fn wasm_module(&self) -> Option<&WasiModule> {
        match self {
            Self::Resource(v) => Some(&v.wasi_module),
            Self::External(v) => Some(&v.wasi_module),
            _ => None,
        }
    }
    pub fn language_id(&self) -> Option<&Identifier> {
        match self {
            Self::Variant(v) => Some(&v.symbol),
            Self::Resource(v) => Some(&v.symbol),
            Self::External(v) => Some(&v.symbol),
            _ => None,
        }
    }
}

impl DependenciesTrace for WasiType {
    #[track_caller]
    fn define_language_types(&self, _: &mut DependentGraph) {
        panic!("Invalid Call");
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        match self {
            WasiType::Option { inner } => inner.collect_wasi_types(dict, collected),
            WasiType::Result { success, failure } => {
                success.iter().for_each(|s| s.collect_wasi_types(dict, collected));
                failure.iter().for_each(|f| f.collect_wasi_types(dict, collected));
            }
            WasiType::Resource(_) => collected.push(self),
            WasiType::Variant(_) => collected.push(self),
            WasiType::TypeAlias { name } => collected.extend(dict.types.get(name)),
            WasiType::TypeHandler { name, .. } => collected.extend(dict.types.get(name)),
            WasiType::External(_) => collected.push(self),
            _ => {}
        };
    }
}
