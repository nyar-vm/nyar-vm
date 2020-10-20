#[allow(non_camel_case_types)]
#[allow(non_upper_case_globals)]
mod flags;
mod prototype;

pub use self::{
    flags::NyarReadWrite,
    prototype::{Class, NyarClass},
};

use crate::typing::Typing;
use std::{
    collections::{BTreeMap, HashMap},
    rc::Rc,
    task::Context,
};
