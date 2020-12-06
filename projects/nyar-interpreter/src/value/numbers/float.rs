use super::*;
use num::Float;
use std::{
    fmt::LowerExp,
    hash::Hasher,
    ops::{Deref, DerefMut},
    str::FromStr,
};

/// A type that wraps a floating-point number and implements [`Hash`].
///
/// Not available under `no_float`.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct FloatWrapper<F>(F);

// impl<F: Float> Hash for FloatWrapper<F> {
//     #[inline(always)]
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         self.0.to_ne_bytes().hash(state);
//     }
// }

macro_rules! eq_hash {
    ($t:ty) => {
        impl Eq for FloatWrapper<$t> {}
        impl Hash for FloatWrapper<$t> {
            #[inline(always)]
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.to_ne_bytes().hash(state);
            }
        }
    };
}

eq_hash!(f32);
eq_hash!(f64);

impl<F: Float> AsRef<F> for FloatWrapper<F> {
    #[inline(always)]
    fn as_ref(&self) -> &F {
        &self.0
    }
}

impl<F: Float> AsMut<F> for FloatWrapper<F> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut F {
        &mut self.0
    }
}

impl<F: Float> Deref for FloatWrapper<F> {
    type Target = F;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F: Float> DerefMut for FloatWrapper<F> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<F: Float + Debug> Debug for FloatWrapper<F> {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<F: Float + Display + LowerExp + From<f32>> Display for FloatWrapper<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.abs();
        if abs > Self::MAX_NATURAL_FLOAT_FOR_DISPLAY.into() || abs < Self::MIN_NATURAL_FLOAT_FOR_DISPLAY.into() {
            write!(f, "{:e}", self.0)
        }
        else {
            Display::fmt(&self.0, f)?;
            if abs.fract().is_zero() {
                f.write_str(".0")?;
            }
            Ok(())
        }
    }
}

impl<F: Float> From<F> for FloatWrapper<F> {
    #[inline(always)]
    fn from(value: F) -> Self {
        Self::new(value)
    }
}

impl<F: Float + FromStr> FromStr for FloatWrapper<F> {
    type Err = <F as FromStr>::Err;

    #[inline(always)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        F::from_str(s).map(Into::<Self>::into)
    }
}

impl<F: Float> FloatWrapper<F> {
    /// Maximum floating-point number for natural display before switching to scientific notation.
    pub const MAX_NATURAL_FLOAT_FOR_DISPLAY: f32 = 10000000000000.0;

    /// Minimum floating-point number for natural display before switching to scientific notation.
    pub const MIN_NATURAL_FLOAT_FOR_DISPLAY: f32 = 0.0000000000001;

    /// Create a new [`FloatWrapper`].
    #[inline(always)]
    pub const fn new(value: F) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub const fn unwrap(self) -> F {
        self.0
    }
}
