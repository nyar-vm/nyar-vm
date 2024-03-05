use std::hash::{DefaultHasher, Hash};

use crate::{
    helpers::{AliasOuter, ComponentDefine, TypeReference, TypeReferenceInput, TypeReferenceOutput},
    WasiParameter,
};

use super::*;

impl Debug for WasiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boolean => write!(f, "bool"),
            Self::Integer8 { signed } => write!(f, "{}8", if *signed { "i" } else { "u" }),
            Self::Integer16 { signed } => write!(f, "{}16", if *signed { "i" } else { "u" }),
            Self::Integer32 { signed } => write!(f, "{}32", if *signed { "i" } else { "u" }),
            Self::Integer64 { signed } => write!(f, "{}64", if *signed { "i" } else { "u" }),
            Self::Option { inner } => write!(f, "Option<{:?}>", inner),
            Self::Result { success: _, failure: _ } => write!(f, "Result<?, ?>"),
            Self::Resource(v) => write!(f, "Resource({})", v.symbol),
            Self::Record(v) => Debug::fmt(v, f),
            Self::Variant(v) => Debug::fmt(v, f),
            Self::TypeHandler(v) => Debug::fmt(v, f),

            Self::External(v) => write!(f, "External({})", v.symbol),
            Self::Array { .. } => {
                write!(f, "Array(..))")
            }
            WasiType::Float32 => {
                write!(f, "f32")
            }
            WasiType::Float64 => {
                write!(f, "f64")
            }
        }
    }
}

impl Display for WasiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WasiType::Boolean => write!(f, "bool"),
            Self::Integer8 { signed } => write!(f, "{}", if *signed { "s" } else { "u" }),
            Self::Integer16 { signed } => write!(f, "{}", if *signed { "s" } else { "u" }),
            Self::Integer32 { signed } => write!(f, "{}", if *signed { "s" } else { "u" }),
            Self::Integer64 { signed } => write!(f, "{}", if *signed { "s" } else { "u" }),
            Self::Option { inner } => write!(f, "Option<{}>", inner),
            Self::Result { success: _, failure: _ } => write!(f, "Result<?, ?>"),
            Self::Resource(v) => write!(f, "Resource({})", v.symbol),
            Self::Record(v) => Debug::fmt(v, f),
            Self::Variant(v) => Debug::fmt(v, f),
            Self::TypeHandler(v) => Debug::fmt(v, f),
            Self::External(v) => write!(f, "External({})", v.symbol),
            Self::Array { .. } => {
                write!(f, "Array(..))")
            }
            WasiType::Float32 => {
                write!(f, "f32")
            }
            WasiType::Float64 => {
                write!(f, "f64")
            }
        }
    }
}

impl ComponentDefine for WasiType {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Variant(v) => v.component_define(w),
            Self::Record(v) => v.component_define(w),
            _ => panic!("This type cannot be defined in the wasi component section\n    {self}"),
        }
    }
}

impl TypeReferenceInput for WasiParameter {
    fn upper_input<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(param \"{}\" ", self.wasi_name)?;
        self.r#type.upper_type(w)?;
        write!(w, ")")
    }

    fn lower_input<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(param ${} ", self.name)?;
        self.r#type.lower_type(w)?;
        write!(w, ")")
    }
}

impl TypeReferenceOutput for WasiType {
    fn upper_output<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(result ")?;
        self.upper_type(w)?;
        write!(w, ")")
    }

    fn lower_output<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(result ")?;
        self.lower_type(w)?;
        write!(w, ")")
    }
}

impl TypeReference for WasiType {
    fn upper_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean => w.write_str("bool")?,
            Self::Integer8 { signed } => match *signed {
                true => w.write_str("s8"),
                false => w.write_str("u8"),
            }?,
            Self::Integer16 { signed } => match *signed {
                true => w.write_str("s16"),
                false => w.write_str("u16"),
            }?,
            Self::Integer32 { signed } => match *signed {
                true => w.write_str("s32"),
                false => w.write_str("u32"),
            }?,
            Self::Integer64 { signed } => match *signed {
                true => w.write_str("s64"),
                false => w.write_str("u64"),
            }?,
            Self::Float32 => w.write_str("float32")?,
            Self::Float64 => w.write_str("float64")?,
            Self::Option { inner } => {
                todo!()
            }
            Self::Result { success, failure } => {
                w.write_str("(result ")?;
                if let Some(s) = success {
                    s.upper_type(w)?
                }
                if let Some(f) = failure {
                    w.write_str("(error ")?;
                    f.upper_type(w)?;
                    w.write_str(")")?;
                }
                w.write_str(")")?
            }
            Self::Resource(_) => {
                todo!()
            }
            Self::Record(v) => v.component_define(w)?,
            Self::Variant(v) => v.component_define(w)?,
            Self::TypeHandler(v) => v.upper_type(w)?,

            Self::External(_) => {
                todo!()
            }
            Self::Array(array) => array.upper_type(w)?,
        }
        Ok(())
    }

    fn lower_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean => w.write_str("i32")?,
            Self::Integer8 { .. } => w.write_str("i32")?,
            Self::Integer16 { .. } => w.write_str("i32")?,
            Self::Integer32 { .. } => w.write_str("i32")?,
            Self::Integer64 { .. } => w.write_str("i64")?,
            Self::Option { .. } => {
                todo!()
            }
            Self::Result { .. } => w.write_str("result")?,
            Self::Resource(_) => w.write_str("i32")?,
            Self::Variant(_) => {
                todo!()
            }
            Self::Record(_) => w.write_str("i32")?,
            Self::TypeHandler(v) => w.source.graph.get(v).lower_type(w)?,
            Self::Array(array) => array.lower_type(w)?,
            Self::External(_) => {
                todo!()
            }
            Self::Float32 => {
                todo!()
            }
            Self::Float64 => {
                todo!()
            }
        }
        Ok(())
    }

    fn lower_type_inner<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean => w.write_str("i8")?,
            Self::Integer8 { .. } => w.write_str("i8")?,
            Self::Integer16 { .. } => w.write_str("i16")?,
            Self::Integer32 { .. } => w.write_str("i32")?,
            Self::Integer64 { .. } => w.write_str("i64")?,
            Self::Option { .. } => {
                todo!()
            }
            Self::Result { .. } => w.write_str("result")?,
            Self::Resource(_) => w.write_str("resource")?,
            Self::Record(_) => {
                todo!()
            }
            Self::Variant(_) => {
                todo!()
            }
            Self::TypeHandler(v) => w.source.graph.get(v).lower_type(w)?,
            Self::Array(array) => array.lower_type(w)?,
            Self::External(_) => {
                todo!()
            }
            Self::Float32 => {
                todo!()
            }
            Self::Float64 => {
                todo!()
            }
        }
        Ok(())
    }
}

impl WasiType {
    pub(crate) fn mermaid_plot(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let hash = hasher.finish();
        match self {
            Self::Boolean => "".to_string(),
            Self::Integer8 { .. } => "".to_string(),
            Self::Integer16 { .. } => "".to_string(),
            Self::Integer32 { .. } => "".to_string(),
            Self::Integer64 { .. } => "".to_string(),
            Self::Option { .. } => "".to_string(),
            Self::Result { .. } => "".to_string(),
            Self::Resource(v) => {
                format!("hash{}[\"{}\"]:::resource", hash, self)
            }
            Self::Variant(_) => "".to_string(),
            Self::TypeHandler { .. } => "".to_string(),
            Self::External(_) => "".to_string(),
            Self::Array { .. } => {
                todo!()
            }
            Self::Float32 => {
                todo!()
            }
            Self::Float64 => {
                todo!()
            }
            WasiType::Record(_) => {
                todo!()
            }
        }
    }
}

impl AliasOuter for WasiType {
    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Resource(v) => v.alias_outer(w),
            Self::Record(v) => v.alias_outer(w),
            Self::Variant(v) => v.alias_outer(w),
            _ => panic!("This type cannot be imported into component instance\n    {self}"),
        }
    }
}
