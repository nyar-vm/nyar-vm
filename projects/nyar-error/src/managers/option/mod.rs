use std::sync::Arc;

use crate::{types::ValkyrieMetaType, ValkyrieTypeLegacy, ValkyrieValue, ValkyrieVariantType};

impl<T> ValkyrieTypeLegacy for Option<T>
where
    T: ValkyrieTypeLegacy + 'static,
{
    fn boxed(self) -> ValkyrieValue {
        let this = ValkyrieVariantType::new("std.primitive.Option".to_string());
        this.boxed()
    }
    fn type_info(&self) -> Arc<ValkyrieMetaType>
    where
        Self: Sized,
    {
        unimplemented!()
    }
}
