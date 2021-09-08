use crate::{types::ValkyrieMetaType, ValkyrieValue};
use std::sync::Arc;

pub trait ValkyrieFunction {
    fn boxed(self) -> ValkyrieValue;
    fn type_info(&self) -> Arc<ValkyrieMetaType>;
}
