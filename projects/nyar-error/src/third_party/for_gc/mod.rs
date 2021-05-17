use shredder::{
    wrappers::{GcPoisonError, GcRwLockReadGuard},
    Scan,
};

use crate::NyarError;

impl<'a, T: Scan + 'static> From<GcPoisonError<GcRwLockReadGuard<'a, T>>> for NyarError {
    fn from(_: GcPoisonError<GcRwLockReadGuard<'a, T>>) -> Self {
        NyarError::read_write_error("Cannot read value from lock")
    }
}
