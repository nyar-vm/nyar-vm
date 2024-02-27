use std::{
    fmt::Write,
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{WasiParameter, WastEncoder};

use super::*;

impl Debug for WasiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer8 { signed } => write!(f, "{}8", if *signed { "i" } else { "u" }),
            Self::Integer16 { signed } => write!(f, "{}16", if *signed { "i" } else { "u" }),
            Self::Integer32 { signed } => write!(f, "{}32", if *signed { "i" } else { "u" }),
            Self::Integer64 { signed } => write!(f, "{}64", if *signed { "i" } else { "u" }),
            Self::Option { inner } => write!(f, "Option<{:?}>", inner),
            Self::Result { success: _, failure: _ } => write!(f, "Result<?, ?>"),
            Self::Resource(v) => write!(f, "Resource({})", v.symbol),
            Self::Variant(v) => Debug::fmt(v, f),
            Self::TypeHandler { name, own } => write!(f, "TypeHandler({}{})", name, if *own { " own" } else { "" }),
            Self::TypeAlias { name } => write!(f, "TypeAlias({})", name),
            Self::External(v) => write!(f, "External({})", v.symbol),
            Self::Array { .. } => {
                write!(f, "Array(..))")
            }
        }
    }
}

impl Display for WasiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer8 { signed } => write!(f, "{}", if *signed { "s" } else { "u" }),
            Self::Integer16 { signed } => write!(f, "{}", if *signed { "s" } else { "u" }),
            Self::Integer32 { signed } => write!(f, "{}", if *signed { "s" } else { "u" }),
            Self::Integer64 { signed } => write!(f, "{}", if *signed { "s" } else { "u" }),
            Self::Option { inner } => write!(f, "Option<{}>", inner),
            Self::Result { success: _, failure: _ } => write!(f, "Result<?, ?>"),
            Self::Resource(v) => write!(f, "Resource({})", v.symbol),
            Self::Variant(v) => Debug::fmt(v, f),
            Self::TypeHandler { name, own } => write!(f, "TypeHandler({}{})", name, if *own { " own" } else { "" }),
            Self::TypeAlias { name } => write!(f, "TypeAlias({})", name),
            Self::External(v) => write!(f, "External({})", v.symbol),
            Self::Array { .. } => {
                write!(f, "Array(..))")
            }
        }
    }
}

impl ComponentDefine for WasiType {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Variant(v) => v.component_define(w),
            _ => panic!("This type cannot be defined in the wasi component section\n    {self}"),
        }
    }
}

impl WasiType {
    pub(crate) fn write_wasi_result<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(result ")?;
        self.upper_type(w)?;
        w.write_str(")")
    }
}

impl TypeReferenceInput for WasiParameter {
    fn upper_input<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(param ${} ", self.name)?;
        self.r#type.lower_type(w)?;
        w.write_str(")")
    }

    fn lower_input<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}
impl TypeReferenceOutput for WasiType {
    fn upper_output<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }

    fn lower_output<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(result ")?;
        self.lower_type(w)?;
        w.write_str(")")
    }
}
impl TypeReference for WasiType {
    fn upper_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Integer8 { signed } => match *signed {
                true => w.write_str("i8"),
                false => w.write_str("u8"),
            }?,
            Self::Integer16 { signed } => match *signed {
                true => w.write_str("i16"),
                false => w.write_str("u16"),
            }?,
            Self::Integer32 { signed } => match *signed {
                true => w.write_str("i32"),
                false => w.write_str("u32"),
            }?,
            Self::Integer64 { signed } => match *signed {
                true => w.write_str("i64"),
                false => w.write_str("u64"),
            }?,
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
            Self::Variant(v) => v.component_define(w)?,
            Self::TypeHandler { name, own } => match own {
                true => write!(w, "(own {})", name.wasi_id())?,
                false => write!(w, "(borrow {})", name.wasi_id())?,
            },
            Self::TypeAlias { name } => w.write_str(&name.wasi_id())?,
            Self::External(_) => {
                todo!()
            }
            Self::Array { inner } => {
                w.write_str("(list ")?;
                inner.upper_type(w)?;
                w.write_str(")")?
            }
        }
        Ok(())
    }

    fn lower_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Integer8 { .. } => w.write_str("i32")?,
            Self::Integer16 { .. } => w.write_str("i32")?,
            Self::Integer32 { .. } => w.write_str("i32")?,
            Self::Integer64 { .. } => w.write_str("i32")?,
            Self::Option { .. } => {
                todo!()
            }
            Self::Result { .. } => {
                todo!()
            }
            Self::Resource(_) => {
                todo!()
            }
            Self::Variant(_) => {
                todo!()
            }
            Self::TypeHandler { .. } => {
                todo!()
            }
            Self::Array { .. } => {
                todo!()
            }
            Self::TypeAlias { .. } => {
                todo!()
            }
            Self::External(_) => {
                todo!()
            }
        }
        Ok(())
    }
}

impl WasiType {
    pub fn as_language_type(&self) -> String {
        match self {
            Self::Integer8 { signed } => match *signed {
                true => "s8",
                false => "u8",
            }
            .to_string(),
            Self::Integer16 { signed } => match *signed {
                true => "s16",
                false => "u16",
            }
            .to_string(),
            Self::Integer32 { signed } => match *signed {
                true => "s32",
                false => "u32",
            }
            .to_string(),
            Self::Integer64 { signed } => match *signed {
                true => "s64",
                false => "u64",
            }
            .to_string(),
            Self::Option { inner } => format!("Option<{}>", inner.as_language_type()),
            Self::Result { success, failure } => {
                let mut result = "(result ".to_string();
                if let Some(success) = success {
                    result.push_str(&success.as_language_type());
                }
                if let Some(failure) = failure {
                    result.push_str(&format!(" (error {})", failure.as_language_type()));
                }
                result.push(')');
                result
            }
            Self::Resource(_) => "(sub resource)".to_string(),
            Self::Variant(_) => "(variant case)".to_string(),
            Self::TypeHandler { name, own } => match *own {
                true => format!("{}", name),
                false => format!("&{}", name),
            },
            Self::TypeAlias { name } => format!("{}", name),
            Self::External(_) => "(func external)".to_string(),
            WasiType::Array { .. } => {
                todo!()
            }
        }
    }
}

impl WasiType {
    pub(crate) fn mermaid_plot(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let hash = hasher.finish();
        match self {
            WasiType::Integer8 { .. } => "".to_string(),
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
            Self::TypeAlias { .. } => "".to_string(),
            Self::External(_) => "".to_string(),
            WasiType::Array { .. } => {
                todo!()
            }
        }
    }
}

impl AliasOuter for WasiType {
    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Resource(v) => v.alias_outer(w),
            Self::Variant(v) => v.alias_outer(w),
            _ => panic!("This type cannot be imported into component instance\n    {self}"),
        }
    }
}
