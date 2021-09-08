use std::sync::Arc;

use crate::{types::ValkyrieMetaType, ValkyrieClass, ValkyrieTypeLegacy, ValkyrieValue};

impl ValkyrieTypeLegacy for u8 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Unsigned8(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Unsigned8");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for u16 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Unsigned16(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Unsigned16");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for u32 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Unsigned32(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Unsigned32");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for u64 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Unsigned64(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Unsigned64");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for u128 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Unsigned128(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Unsigned128");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for usize {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Unsigned64(self as u64)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Unsigned64");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for i8 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Integer8(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Integer8");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for i16 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Integer16(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Integer16");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for i32 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Integer32(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Integer32");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for i64 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Integer64(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Integer64");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for i128 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Integer128(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Integer128");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for isize {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Integer64(self as i64)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Integer64");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for f32 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Float32(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Float32");
        Arc::new(this)
    }
}

impl ValkyrieTypeLegacy for f64 {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Float64(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Float64");
        Arc::new(this)
    }
}

// impl<T> ValkyrieType for Arc<T> {
//     fn type_info() -> ValkyrieMetaType
//         where
//             Self: Sized,
//     {
//         let mut meta = ValkyrieMetaType::default();
//         meta.set_namepath("core.Arc");
//         meta
//     }
// }

impl ValkyrieTypeLegacy for String {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::String(Arc::new(self))
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType>
    where
        Self: Sized,
    {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.UTF8Text");
        Arc::new(this)
    }
}
