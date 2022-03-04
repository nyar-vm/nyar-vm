use std::{ops::Deref, sync::Arc};

use crate::{types::ValkyrieMetaType, ValkyrieTypeLegacy, ValkyrieValue};

impl<T> ValkyrieTypeLegacy for Arc<T>
where
    T: ValkyrieTypeLegacy,
{
    #[track_caller]
    fn boxed(self) -> ValkyrieValue {
        panic!("Arc<T> can't be not boxed")
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        self.as_ref().type_info()
    }
}
