use shredder::{
    wrappers::{GcPoisonError, GcRwLockReadGuard},
    Scan,
};

use crate::NyarError3;

impl<'a, T: Scan + 'static> From<GcPoisonError<GcRwLockReadGuard<'a, T>>> for NyarError3 {
    fn from(_: GcPoisonError<GcRwLockReadGuard<'a, T>>) -> Self {
        NyarError3::read_write_error("Cannot read value from lock")
    }
}
