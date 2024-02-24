use std::fmt::Write;

use super::*;

impl Display for WasiType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            match self {
                Self::Integer8 { signed } => match *signed {
                    true => f.write_str("i8"),
                    false => f.write_str("u8"),
                }?,
                Self::Integer16 { signed } => match *signed {
                    true => f.write_str("i16"),
                    false => f.write_str("u16"),
                }?,
                Self::Integer32 { signed } => match *signed {
                    true => f.write_str("i32"),
                    false => f.write_str("u32"),
                }?,
                Self::Integer64 { signed } => match *signed {
                    true => f.write_str("i64"),
                    false => f.write_str("u64"),
                }?,
                Self::Option { .. } => {
                    panic!()
                }
                Self::Result { .. } => {
                    panic!()
                }
                Self::Resource(_) => {
                    panic!()
                }
                Self::Variant(_) => {
                    panic!()
                }
                Self::TypeHandler { name, own } => match *own {
                    true => write!(f, "{}", name),
                    false => write!(f, "&{}", name),
                }?,
                Self::TypeAlias { name } => write!(f, "{}", name)?,
                Self::External(_) => {
                    panic!()
                }
            }
        }
        else {
            match self {
                Self::Integer8 { signed } => match *signed {
                    true => f.write_str("s8"),
                    false => f.write_str("u8"),
                }?,
                Self::Integer16 { signed } => match *signed {
                    true => f.write_str("s16"),
                    false => f.write_str("u16"),
                }?,
                Self::Integer32 { signed } => match *signed {
                    true => f.write_str("s32"),
                    false => f.write_str("u32"),
                }?,
                Self::Integer64 { signed } => match *signed {
                    true => f.write_str("s64"),
                    false => f.write_str("u64"),
                }?,
                Self::Option { .. } => {
                    panic!()
                }
                // (result <valtype>? (error <valtype>)?)
                Self::Result { success, failure } => {
                    f.write_str("(result ")?;
                    if let Some(success) = success {
                        write!(f, "{}", success)?
                    };
                    if let Some(failure) = failure {
                        write!(f, "(error {})", failure)?
                    };
                    f.write_char(')')?
                }
                Self::Resource(_) => f.write_str("(sub resource)")?,
                Self::Variant(_) => f.write_str("(variant case)")?,
                Self::TypeHandler { name, own } => match *own {
                    true => write!(f, "(own {})", name.wasi_id())?,
                    false => write!(f, "(borrow {})", name.wasi_id())?,
                },
                Self::TypeAlias { name } => write!(f, "${}", name)?,

                Self::External(_) => f.write_str("(func external)")?,
            }
        }
        Ok(())
    }
}
