use num::complex::Complex as ComplexType;
use num::BigInt;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct ComplexPair<R, I> {
    pub re: R,
    pub im: I,
}

#[allow(type_alias_bounds)]
pub type Complex<R: Any, I: Any> = ComplexPair<R, I>;
pub type GaussianInteger = ComplexType<BigInt>;
pub type Complex32 = ComplexType<f32>;
pub type Complex64 = ComplexType<f64>;
