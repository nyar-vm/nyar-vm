pub(crate) use crate::lowering::{
    features::pattern_matching_contract,
    sanitize_symbol,
    shared::{executable, interop},
};

#[cfg(feature = "ovm-lane")]
#[doc(hidden)]
pub mod ovm;
#[cfg(feature = "ovm-lane")]
#[path = "ovm/mir.rs"]
pub(crate) mod ovm_mir;

#[doc(hidden)]
pub mod wasm;

#[doc(hidden)]
pub mod clr;
#[doc(hidden)]
pub mod jvm;
#[doc(hidden)]
pub mod native;
