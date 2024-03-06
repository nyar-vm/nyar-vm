use std::{
    fmt::{Debug, Display, Formatter, Write},
    hash::{Hash, Hasher},
    sync::Arc,
};

use indexmap::IndexMap;

use crate::{
    dag::DependentGraph,
    encoder::WastEncoder,
    helpers::{AliasOuter, ComponentDefine, EmitDefault, TypeDefinition, TypeReference},
    wasi_types::{array::WasiArrayType, functions::WasiFunctionBody, resources::WasiResource, variants::WasiVariantType},
    DependenciesTrace, Identifier, WasiFunction, WasiModule, WasiRecordType, WasiTypeReference,
};
use std::{cmp::Ordering, ops::AddAssign};
use WasiType::{Boolean, Float32, Float64, Integer16, Integer32, Integer64, Integer8};

pub mod array;
mod display;
pub mod enumerations;
pub mod functions;
pub mod records;
pub mod reference;
pub mod resources;
pub mod variants;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasiType {
    Boolean,
    /// `s8` or `u8` type in WASI
    Integer8 {
        /// Whether the integer is signed or not
        signed: bool,
    },
    /// `s16` or `u16` type in WASI
    Integer16 {
        /// Whether the integer is signed or not
        signed: bool,
    },
    /// `s32` or `u32` type in WASI
    Integer32 {
        /// Whether the integer is signed or not
        signed: bool,
    },
    /// `s64` or `u64` type in WASI
    Integer64 {
        /// Whether the integer is signed or not
        signed: bool,
    },
    /// `f32` type in WASI
    Float32,
    /// `f64` type in WASI
    Float64,
    Option {
        inner: Box<WasiType>,
    },
    Result {
        success: Option<Box<WasiType>>,
        failure: Option<Box<WasiType>>,
    },
    /// `resource` type in WASI
    Resource(WasiResource),
    /// `record` type in WASI
    Record(WasiRecordType),
    /// `variant` type in WASI
    Variant(WasiVariantType),
    /// type reference in WASI
    TypeHandler(WasiTypeReference),
    /// `list` type in WASI
    Array(Box<WasiArrayType>),
    /// `function` type in WASI
    Function(Box<WasiFunction>),
}

impl WasiType {
    /// Get the type definition of the type, composite type returns `None`
    pub fn wasm_module(&self) -> Option<&WasiModule> {
        match self {
            Self::Resource(v) => Some(&v.wasi_module),
            Self::Function(v) => match &v.body {
                WasiFunctionBody::External { wasi_module, .. } => Some(wasi_module),
                WasiFunctionBody::Normal { .. } => None,
            },
            _ => None,
        }
    }
    /// Returns the language identifier of the type, anonymous type returns `None`
    pub fn language_id(&self) -> Option<&Identifier> {
        match self {
            Self::Variant(v) => Some(&v.symbol),
            Self::Resource(v) => Some(&v.symbol),
            Self::Function(v) => Some(&v.symbol),
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
            WasiType::TypeHandler(v) => collected.push(dict.get(v)),
            WasiType::Function(_) => collected.push(self),
            _ => {}
        };
    }
}

impl EmitDefault for WasiType {
    fn emit_default<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Boolean => {
                write!(w, "i32.const 0")
            }
            Integer8 { .. } => {
                write!(w, "i32.const 0")
            }
            Integer16 { .. } => {
                write!(w, "i32.const 0")
            }
            Integer32 { .. } => {
                write!(w, "i32.const 0")
            }
            Integer64 { .. } => {
                write!(w, "i64.const 0")
            }
            Float32 => {
                write!(w, "f32.const 0")
            }
            Float64 => {
                write!(w, "f64.const 0")
            }
            Self::Option { .. } => {
                todo!()
            }
            Self::Result { .. } => {
                todo!()
            }
            Self::Resource(_) => {
                todo!()
            }
            Self::Record(_) => {
                todo!()
            }
            Self::Variant(_) => {
                todo!()
            }
            Self::TypeHandler { .. } => {
                todo!()
            }
            Self::Array(_) => {
                todo!()
            }
            Self::Function(_) => {
                todo!()
            }
        }
    }
}

impl WasiType {
    pub(crate) fn emit_convert<W: Write>(&self, target: &WasiType, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Boolean => match target {
                Boolean { .. } | Integer8 { .. } | Integer16 { .. } | Integer32 { .. } => write!(w, "nop")?,
                Integer64 { .. } => write!(w, "i64.extend_i32_u")?,
                Float32 => write!(w, "f32.convert_i32_u")?,
                Float64 => write!(w, "f64.convert_i32_u")?,
                _ => unreachable!("convert {} to {}", self, target),
            },
            Integer8 { signed } | Integer16 { signed } | Integer32 { signed } => match target {
                Boolean { .. } | Integer8 { .. } | Integer16 { .. } | Integer32 { .. } => write!(w, "nop")?,
                Integer64 { .. } if signed => write!(w, "i64.extend_i32_s")?,
                Integer64 { .. } => write!(w, "i64.extend_i32_u")?,
                Float32 if signed => write!(w, "f32.convert_i32_s")?,
                Float32 => write!(w, "f32.convert_i32_u")?,
                Float64 if signed => write!(w, "f64.convert_i32_s")?,
                Float64 => write!(w, "f64.convert_i32_u")?,
                _ => unreachable!("convert {} to {}", self, target),
            },
            Integer64 { signed } => match target {
                Boolean { .. } | Integer8 { .. } | Integer16 { .. } | Integer32 { .. } => write!(w, "i32.wrap_i64")?,
                Integer64 { .. } => write!(w, "nop")?,
                Float32 if signed => write!(w, "f32.convert_i64_s")?,
                Float32 => write!(w, "f32.convert_i64_u")?,
                Float64 if signed => write!(w, "f64.convert_i64_s")?,
                Float64 => write!(w, "f64.convert_i64_u")?,
                _ => unreachable!("convert {} to {}", self, target),
            },
            Float32 => match target {
                Boolean { .. } | Integer8 { signed: true } | Integer16 { signed: true } | Integer32 { signed: true } => {
                    write!(w, "i32.trunc_f32_s")?
                }
                Boolean { .. } | Integer8 { signed: false } | Integer16 { signed: false } | Integer32 { signed: false } => {
                    write!(w, "i32.trunc_f32_u")?
                }
                Integer64 { signed: true } => write!(w, "i64.trunc_f32_s")?,
                Integer64 { signed: false } => write!(w, "i64.trunc_f32_u")?,
                Float32 => write!(w, "nop")?,
                Float64 => write!(w, "f64.promote_f32")?,
                _ => unreachable!("convert {} to {}", self, target),
            },
            Float64 => match target {
                Boolean { .. } | Integer8 { signed: true } | Integer16 { signed: true } | Integer32 { signed: true } => {
                    write!(w, "i32.trunc_f64_s")?
                }
                Boolean { .. } | Integer8 { signed: false } | Integer16 { signed: false } | Integer32 { signed: false } => {
                    write!(w, "i32.trunc_f64_u")?
                }
                Integer64 { signed: true } => write!(w, "i64.trunc_f64_s")?,
                Integer64 { signed: false } => write!(w, "i64.trunc_f64_u")?,
                Float32 => write!(w, "f32.demote_f64")?,
                Float64 => write!(w, "nop")?,
                _ => unreachable!("convert {} to {}", self, target),
            },
            _ => unreachable!("convert {} to {}", self, target),
        }
        Ok(())
    }
    pub(crate) fn emit_transmute<W: Write>(&self, target: &WasiType, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match (self, target) {
            (Float32, Integer32 { .. }) => write!(w, "i32.reinterpret_f32")?,
            (Float64, Integer64 { .. }) => write!(w, "i64.reinterpret_f64")?,
            (Integer32 { .. }, Float32) => write!(w, "f32.reinterpret_i32")?,
            (Integer64 { .. }, Float64) => write!(w, "f64.reinterpret_i64")?,
            _ => unimplemented!("transmute {} to {}", self, target),
        }
        Ok(())
    }
}
