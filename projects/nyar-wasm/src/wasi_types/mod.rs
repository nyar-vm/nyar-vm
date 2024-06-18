use crate::{
    encoder::WastEncoder,
    helpers::{ComponentSections, DependenciesTrace, EmitDefault, TypeReference},
    wasi_types::{
        array::WasiArrayType, flags::WasiFlags, functions::WasiFunctionBody, resources::WasiResource, variants::WasiVariantType,
    },
    DependentGraph, Identifier, WasiEnumeration, WasiFunction, WasiModule, WasiRecordType, WasiSemanticIndex,
    WasiTypeReference,
};
use indexmap::IndexMap;
use std::{
    cmp::Ordering,
    fmt::{Debug, Display, Formatter, Write},
    hash::{Hash, Hasher},
    ops::AddAssign,
    sync::Arc,
};

pub mod array;
mod display;
pub mod enumerations;
pub mod flags;
pub mod functions;
pub mod records;
pub mod reference;
pub mod resources;
pub mod variants;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasiType {
    /// `bool` type in WASI
    Boolean,
    /// `char` type in WASI
    Unicode,
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
        fine: Option<Box<WasiType>>,
        fail: Option<Box<WasiType>>,
    },
    /// `resource` type in WASI
    Resource(WasiResource),
    /// `record` type in WASI
    Record(WasiRecordType),
    /// `variant` type in WASI
    Variant(WasiVariantType),
    /// `enum` type in WASI
    Enums(WasiEnumeration),
    /// `enum` type in WASI
    Flags(WasiFlags),

    /// `list` type in WASI
    Array(Box<WasiArrayType>),
    /// `function` type in WASI
    Function(Box<WasiFunction>),
    /// type reference in WASI
    TypeHandler(WasiTypeReference),
}

impl WasiType {
    /// Get the type definition of the type, composite type returns `None`
    pub fn wasm_module(&self) -> Option<&WasiModule> {
        match self {
            Self::Resource(v) => Some(&v.wasi_module),
            Self::Function(v) => match &v.body {
                WasiFunctionBody::External { wasi_module, .. } => Some(wasi_module),
                WasiFunctionBody::Native { .. } => None,
                WasiFunctionBody::Assembly { .. } => None,
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
    pub fn is_heap_type(&self) -> bool {
        match self {
            Self::Record(_) => true,
            Self::Array(_) => true,
            _ => false,
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
            Self::Option { inner } => inner.collect_wasi_types(dict, collected),
            Self::Result { fine: success, fail: failure } => {
                success.iter().for_each(|s| s.collect_wasi_types(dict, collected));
                failure.iter().for_each(|f| f.collect_wasi_types(dict, collected));
            }
            Self::Resource(_) => collected.push(self),
            Self::Variant(_) => collected.push(self),
            Self::Function(_) => collected.push(self),
            Self::Flags(_) => collected.push(self),
            Self::Enums(_) => collected.push(self),
            Self::Record(v) => v.collect_wasi_types(dict, collected),
            Self::TypeHandler(v) => collected.push(dict.get(v)),
            _ => {}
        };
    }
}

impl EmitDefault for WasiType {
    fn emit_default<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean | Self::Integer8 { .. } | Self::Integer16 { .. } | Self::Integer32 { .. } => {
                w.write_str("i32.const 0")
            }
            Self::Integer64 { .. } => w.write_str("i64.const 0"),
            Self::Float32 => w.write_str("f32.const 0"),
            Self::Float64 => w.write_str("f64.const 0"),
            // U+0020: " "
            Self::Unicode => w.write_str("f64.const 32"),
            Self::Option { .. } => {
                todo!()
            }
            Self::Result { .. } => {
                todo!()
            }
            Self::Resource(_) => {
                todo!()
            }
            Self::Record(v) => v.emit_default(w),
            Self::Variant(_) => {
                todo!()
            }
            Self::TypeHandler { .. } => {
                todo!()
            }
            Self::Array(v) => v.emit_default(w),
            Self::Function(_) => {
                todo!()
            }
            Self::Enums(_) => w.write_str("i32.const 0"),
            Self::Flags(_) => w.write_str("i32.const 0"),
        }
    }
}

impl EmitDefault for WasiArrayType {
    fn emit_default<W: Write>(&self, _: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}
impl EmitDefault for WasiRecordType {
    fn emit_default<W: Write>(&self, _: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}

impl WasiType {
    pub(crate) fn emit_convert<W: Write>(&self, target: &WasiType, w: &mut WastEncoder<W>) -> std::fmt::Result {
        use WasiType::{Boolean, Float32, Float64, Integer16, Integer32, Integer64, Integer8};
        match self {
            Boolean => match target {
                Boolean | Integer8 { .. } | Integer16 { .. } | Integer32 { .. } => write!(w, "nop")?,
                Integer64 { .. } => write!(w, "i64.extend_i32_u")?,
                Float32 => write!(w, "f32.convert_i32_u")?,
                Float64 => write!(w, "f64.convert_i32_u")?,
                _ => unreachable!("Can't convert `{}` to `{}`", self, target),
            },
            Integer8 { signed } | Integer16 { signed } | Integer32 { signed } => match target {
                Boolean | Integer8 { .. } | Integer16 { .. } | Integer32 { .. } => write!(w, "nop")?,
                Integer64 { .. } if *signed => write!(w, "i64.extend_i32_s")?,
                Integer64 { .. } => write!(w, "i64.extend_i32_u")?,
                Float32 if *signed => write!(w, "f32.convert_i32_s")?,
                Float32 => write!(w, "f32.convert_i32_u")?,
                Float64 if *signed => write!(w, "f64.convert_i32_s")?,
                Float64 => write!(w, "f64.convert_i32_u")?,
                _ => unreachable!("Can't convert `{}` to `{}`", self, target),
            },
            Integer64 { signed } => match target {
                Boolean | Integer8 { .. } | Integer16 { .. } | Integer32 { .. } => write!(w, "i32.wrap_i64")?,
                Integer64 { .. } => write!(w, "nop")?,
                Float32 if *signed => write!(w, "f32.convert_i64_s")?,
                Float32 => write!(w, "f32.convert_i64_u")?,
                Float64 if *signed => write!(w, "f64.convert_i64_s")?,
                Float64 => write!(w, "f64.convert_i64_u")?,
                _ => unreachable!("Can't convert `{}` to `{}`", self, target),
            },
            Float32 => match target {
                Boolean | Integer8 { signed: true } | Integer16 { signed: true } | Integer32 { signed: true } => {
                    write!(w, "i32.trunc_f32_s")?
                }
                Integer8 { signed: false } | Integer16 { signed: false } | Integer32 { signed: false } => {
                    write!(w, "i32.trunc_f32_u")?
                }
                Integer64 { signed: true } => write!(w, "i64.trunc_f32_s")?,
                Integer64 { signed: false } => write!(w, "i64.trunc_f32_u")?,
                Float32 => write!(w, "nop")?,
                Float64 => write!(w, "f64.promote_f32")?,
                _ => unreachable!("Can't convert `{}` to `{}`", self, target),
            },
            Float64 => match target {
                Boolean | Integer8 { signed: true } | Integer16 { signed: true } | Integer32 { signed: true } => {
                    write!(w, "i32.trunc_f64_s")?
                }
                Integer8 { signed: false } | Integer16 { signed: false } | Integer32 { signed: false } => {
                    write!(w, "i32.trunc_f64_u")?
                }
                Integer64 { signed: true } => write!(w, "i64.trunc_f64_s")?,
                Integer64 { signed: false } => write!(w, "i64.trunc_f64_u")?,
                Float32 => write!(w, "f32.demote_f64")?,
                Float64 => write!(w, "nop")?,
                _ => unreachable!("Can't convert `{}` to `{}`", self, target),
            },
            _ => unimplemented!("Unknown convert {} to {}", self, target),
        }
        Ok(())
    }
    pub(crate) fn emit_transmute<W: Write>(&self, target: &WasiType, w: &mut WastEncoder<W>) -> std::fmt::Result {
        use WasiType::{Float32, Float64, Integer32, Integer64};
        match (self, target) {
            (Float32, Integer32 { .. }) => write!(w, "i32.reinterpret_f32")?,
            (Float64, Integer64 { .. }) => write!(w, "i64.reinterpret_f64")?,
            (Integer32 { .. }, Float32) => write!(w, "f32.reinterpret_i32")?,
            (Integer64 { .. }, Float64) => write!(w, "f64.reinterpret_i64")?,
            _ => unimplemented!("Unknown transmute {} to {}", self, target),
        }
        Ok(())
    }
}
