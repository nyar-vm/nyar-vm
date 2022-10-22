use std::hash::{DefaultHasher, Hash, Hasher};

use super::*;

impl Debug for WasiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer8 { signed } => write!(f, "{}8", if *signed { "i" } else { "u" }),
            Self::Integer16 { signed } => write!(f, "{}16", if *signed { "i" } else { "u" }),
            Self::Integer32 { signed } => write!(f, "{}32", if *signed { "i" } else { "u" }),
            Self::Integer64 { signed } => write!(f, "{}64", if *signed { "i" } else { "u" }),
            Self::Option { inner } => write!(f, "Option<{}>", inner),
            Self::Result { success: _, failure: _ } => write!(f, "Result<?, ?>"),
            Self::Resource(v) => write!(f, "Resource({})", v.symbol),
            Self::Variant(v) => Debug::fmt(v, f),
            Self::TypeHandler { name, own } => write!(f, "TypeHandler({}{})", name, if *own { " own" } else { "" }),
            Self::TypeAlias { name } => write!(f, "TypeAlias({})", name),
            Self::External(v) => write!(f, "External({})", v.symbol),
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
            Self::Option { inner } => write!(f, "Option<{}>", inner.as_wasi_type()),
            Self::Result { success: _, failure: _ } => write!(f, "Result<?, ?>"),
            Self::Resource(v) => write!(f, "Resource({})", v.symbol),
            Self::Variant(v) => Debug::fmt(v, f),
            Self::TypeHandler { name, own } => write!(f, "TypeHandler({}{})", name, if *own { " own" } else { "" }),
            Self::TypeAlias { name } => write!(f, "TypeAlias({})", name),
            Self::External(v) => write!(f, "External({})", v.symbol),
        }
    }
}

impl WasiType {
    pub fn as_wasi_type(&self) -> String {
        match self {
            Self::Integer8 { signed } => match *signed {
                true => "i8",
                false => "u8",
            }
            .to_string(),
            Self::Integer16 { signed } => match *signed {
                true => "i16",
                false => "u16",
            }
            .to_string(),
            Self::Integer32 { signed } => match *signed {
                true => "i32",
                false => "u32",
            }
            .to_string(),
            Self::Integer64 { signed } => match *signed {
                true => "i64",
                false => "u64",
            }
            .to_string(),
            Self::Option { inner } => format!("Option<{}>", inner.as_wasi_type()),
            Self::Result { success, failure } => {
                let mut result = "(result ".to_string();
                if let Some(success) = success {
                    result.push_str(&success.as_wasi_type());
                }
                if let Some(failure) = failure {
                    result.push_str(&format!(" (error {})", failure.as_wasi_type()));
                }
                result.push(')');
                result
            }
            Self::Resource(_) => "(sub resource)".to_string(),
            Self::Variant(_) => "(variant case)".to_string(),
            Self::TypeHandler { name, own } => match *own {
                true => format!("(own {})", name.wasi_id()),
                false => format!("(borrow {})", name.wasi_id()),
            },
            Self::TypeAlias { name } => name.wasi_id(),
            Self::External(_) => "(func external)".to_string(),
        }
    }
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
                format!("hash{}[\"{}\"]:::resource", hash, self.as_wasi_type())
            }
            Self::Variant(_) => "".to_string(),
            Self::TypeHandler { .. } => "".to_string(),
            Self::TypeAlias { .. } => "".to_string(),
            Self::External(_) => "".to_string(),
        }
    }
}
