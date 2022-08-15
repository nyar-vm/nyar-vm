use super::*;

impl From<bool> for WasmValue {
    fn from(value: bool) -> Self {
        match value {
            true => Self::I32(1),
            false => Self::I32(0),
        }
    }
}

macro_rules! value_map {
    ($($type:ty => $variant:ident),*) => {
        $(
            impl From<$type> for WasmValue {
                fn from(value: $type) -> Self {
                    Self::$variant(value)
                }
            }
        )*
    };
}

value_map! {
    u32 => U32,
    i32 => I32,
    i64 => I64,
    f32 => F32,
    f64 => F64
}
