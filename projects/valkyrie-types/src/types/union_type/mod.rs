use super::*;

pub struct ValkyrieUnionType {
    terms: Vec<ValkyrieMetaType>,
}

impl Default for ValkyrieUnionType {
    fn default() -> Self {
        todo!()
    }
}

impl ValkyrieTypeLegacy for ValkyrieUnionType {
    fn boxed(self) -> ValkyrieValue {
        todo!()
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        todo!()
    }
}
