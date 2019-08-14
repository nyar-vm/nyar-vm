mod cast;

use std::sync::Arc;
use crate::NyarClass;
pub use self::cast::{NyarCast};

pub enum NyarValue {
    Class(Arc<dyn NyarClass>)
}