mod cast;
mod dynamic;

pub use self::cast::NyarCast;
use crate::NyarClass;
use std::sync::Arc;

pub enum NyarValue {
    Class(Arc<dyn NyarClass>),
}
