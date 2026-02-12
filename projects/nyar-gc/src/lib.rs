#![warn(missing_docs)]

pub mod block;
pub mod collector;
pub mod ffi;
pub mod object;
pub mod ptr;
pub mod runtime;
pub mod stack;
pub mod tlab;

pub use nyar_macros::Trace;

pub use block::{GcBlock, GcBlockHeader, BLOCK_SIZE};
pub use collector::NyarGc;
pub use ffi::{PersistentRoot, Root};
pub use object::{Gc, GcBox, GcCell, GcHeader, GcState, MarkContext, Trace};
pub use ptr::SendPtr;
pub use runtime::GcRuntime;
pub use tlab::Tlab;
