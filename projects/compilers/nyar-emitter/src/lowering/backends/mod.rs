pub(crate) use crate::lowering::{
    features::{pattern_matching_contract, singleton},
    sanitize_symbol,
    shared::{executable, interop},
};

#[cfg(feature = "nyar-vm-lane")]
pub(crate) mod nyar_vm;
#[cfg(feature = "nyar-vm-lane")]
#[path = "nyar_vm/mir.rs"]
pub(crate) mod nyar_vm_mir;

pub(crate) mod wasm;
