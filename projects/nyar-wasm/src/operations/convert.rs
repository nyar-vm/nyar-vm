use super::*;

impl From<JumpBranch> for Operation {
    fn from(value: JumpBranch) -> Self {
        Self::JumpBranch(value)
    }
}

impl From<JumpTable> for Operation {
    fn from(value: JumpTable) -> Self {
        Self::JumpTable(value)
    }
}

macro_rules! from_value {
    ($($type:ty),*) => {
        $(
            impl From<$type> for Operation {
                fn from(value: $type) -> Self {
                    Self::Constant { value: WasmValue::from(value) }
                }
            }
        )*
    };
}

from_value!(bool, i32, i64, f32, f64);
