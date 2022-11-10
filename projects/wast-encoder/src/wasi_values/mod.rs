use std::fmt::Write;

use crate::{encoder::WastEncoder, WasiArrayType};

pub enum WasiValue {
    Boolean(bool),
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
    Array { r#type: WasiArrayType },
}

impl Default for WasiValue {
    fn default() -> Self {
        todo!()
    }
}

// impl WasiValue {
//
// }
