use super::*;
use crate::WasiRecordType;
use indexmap::IndexMap;
use std::sync::Arc;

pub struct RecordValue {
    pub r#type: WasiRecordType,
    pub values: IndexMap<Arc<str>, WasiValue>,
}

impl EmitConstant for RecordValue {
    fn emit_constant<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "struct.new_default {}", self.r#type.symbol)?;
        // local.set ...
        w.stack.push(WasiType::Record(self.r#type.clone()));

        // struct.set ...

        Ok(())
    }
}
