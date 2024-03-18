use dependent_sort::Task;
use std::hash::{DefaultHasher, Hash};

use crate::{
    helpers::{ComponentDefine, GroupedTask, LowerTypes, TypeReference, TypeReferenceInput, TypeReferenceOutput},
    WasiParameter,
};

use super::*;

impl Debug for WasiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Option { inner } => write!(f, "Option<{:?}>", inner),
            Self::Result { success: _, failure: _ } => write!(f, "Result<?, ?>"),
            Self::Resource(v) => Debug::fmt(v, f),
            Self::Record(v) => Debug::fmt(v, f),
            Self::Variant(v) => Debug::fmt(v, f),
            Self::TypeHandler(v) => Debug::fmt(v, f),
            Self::Array(v) => Debug::fmt(v, f),
            Self::Function(v) => Debug::fmt(v, f),
            _ => Display::fmt(self, f),
        }
    }
}

impl Display for WasiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boolean => f.write_str("bool"),
            Self::Unicode => f.write_str("char"),
            Self::Integer8 { signed } => write!(f, "{}8", if *signed { "i" } else { "u" }),
            Self::Integer16 { signed } => write!(f, "{}16", if *signed { "i" } else { "u" }),
            Self::Integer32 { signed } => write!(f, "{}32", if *signed { "i" } else { "u" }),
            Self::Integer64 { signed } => write!(f, "{}64", if *signed { "i" } else { "u" }),
            Self::Float32 => f.write_str("f32"),
            Self::Float64 => f.write_str("f64"),
            Self::Option { inner } => write!(f, "{}?", inner),
            Self::Result { success: _, failure: _ } => write!(f, "Result<?, ?>"),
            Self::Resource(v) => write!(f, "Resource({})", v.symbol),
            Self::Record(v) => Display::fmt(v, f),
            Self::Variant(v) => Display::fmt(v, f),
            Self::TypeHandler(v) => Display::fmt(v, f),
            Self::Array(v) => Display::fmt(v, f),
            Self::Function(v) => Display::fmt(v, f),
            Self::Enums(v) => Display::fmt(v, f),
            Self::Flags(v) => Display::fmt(v, f),
        }
    }
}

impl GroupedTask for WasiType {
    fn dependent_task<'a, 'b>(&'a self, graph: &'b DependentGraph) -> Option<Task<WasiType, WasiModule>>
    where
        'b: 'a,
    {
        let mut dependents: Vec<&WasiType> = vec![];
        let task = match self {
            Self::Resource(v) => Task::new(self).with_group(&v.wasi_module),
            Self::Record(v) => {
                v.collect_wasi_types(graph, &mut dependents);
                Task::new_with_dependent(self, dependents)
            }
            Self::Variant(v) => {
                v.collect_wasi_types(graph, &mut dependents);
                Task::new_with_dependent(self, dependents)
            }

            Self::Function(v) => match &v.body {
                WasiFunctionBody::External { wasi_module, .. } => {
                    v.collect_wasi_types(graph, &mut dependents);
                    Task { id: self, group: Some(wasi_module), dependent_tasks: dependents }
                }
                WasiFunctionBody::Native { .. } => {
                    v.collect_wasi_types(graph, &mut dependents);
                    Task { id: self, group: None, dependent_tasks: dependents }
                }
            },
            Self::Enums(_) => Task { id: self, group: None, dependent_tasks: dependents },
            Self::Flags(_) => Task { id: self, group: None, dependent_tasks: dependents },
            _ => return None,
        };
        Some(task)
    }
}

impl ComponentDefine for WasiType {
    fn wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Variant(v) => {
                w.newline()?;
                v.wasi_define(w)
            }
            Self::Record(v) => {
                w.newline()?;
                v.wasi_define(w)
            }
            Self::Function(v) => {
                match &v.body {
                    WasiFunctionBody::External { .. } if cfg!(debug_assertions) => {
                        panic!("Imported functions cannot be defined using independent wasi: {v}")
                    }
                    _ => {}
                }
                Ok(())
            }
            Self::Flags(f) => {
                w.newline()?;
                write!(w, "(flags \"{}\" ", f.symbol)
            }
            Self::Enums(f) => {
                w.newline()?;
                write!(w, "(enum \"{}\" ", f.symbol)
            }
            Self::Variant(f) => {
                w.newline()?;
                write!(w, "(variant \"{}\" ", f.symbol)
            }
            _ => panic!("This type cannot be defined in the wasi component section\n    {self}"),
        }
    }

    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Resource(v) => v.alias_outer(w),
            Self::Record(v) => v.alias_outer(w),
            Self::Variant(v) => v.alias_outer(w),
            _ => panic!("This type cannot be imported into component instance\n    {self}"),
        }
    }

    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        todo!()
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
        self.lower_type(w)?;
        write!(w, ")")
    }

    fn lower_output<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(result ")?;
        self.lower_type(w)?;
        write!(w, ")")
    }
}

impl LowerTypes for WasiType {
    fn canon_lower<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Function(f) => f.canon_lower(w),
            _ => Ok(()),
        }
    }

    fn wasm_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Function(f) => f.wasm_define(w),
            _ => Ok(()),
        }
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
            Self::Record(v) => v.wasi_define(w)?,
            Self::Variant(v) => v.wasi_define(w)?,
            Self::TypeHandler(v) => v.upper_type(w)?,
            Self::Function(_) => {
                todo!()
            }
            Self::Array(array) => array.upper_type(w)?,
            Self::Unicode => w.write_str("char")?,
            Self::Enums(_) => {}
            Self::Flags(_) => {}
        }
        Ok(())
    }

    fn lower_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean => w.write_str("i32")?,
            Self::Unicode => w.write_str("i32")?,
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
            Self::Record(v) => write!(w, "(ref eq ${})", v.wasi_name)?,
            Self::TypeHandler(v) => w.source.graph.get(v).canon_lower(w)?,
            Self::Array(array) => array.lower_type(w)?,

            Self::Float32 => {
                todo!()
            }
            Self::Float64 => {
                todo!()
            }
            Self::Function(_) => {
                todo!()
            }

            WasiType::Enums(_) => {
                todo!()
            }
            WasiType::Flags(_) => {
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
            Self::TypeHandler(v) => w.source.graph.get(v).canon_lower(w)?,
            Self::Array(array) => array.lower_type(w)?,

            Self::Float32 => {
                todo!()
            }
            Self::Float64 => {
                todo!()
            }
            Self::Function(_) => {
                todo!()
            }
            Self::Unicode => {}
            _ => {}
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
            Self::Unicode => {
                todo!()
            }
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

            Self::Array { .. } => {
                todo!()
            }
            Self::Float32 => {
                todo!()
            }
            Self::Float64 => {
                todo!()
            }
            Self::Record(_) => {
                todo!()
            }
            Self::Function(_) => {
                todo!()
            }
            Self::Enums(_) => {
                todo!()
            }
            Self::Flags(_) => {
                todo!()
            }
        }
    }
}
