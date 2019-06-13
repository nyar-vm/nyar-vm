










```rust
type Real = Integer | Decimal | Rational;
struct Complex {
    re: Real,
    im: Real,
}
impl Add<Complex> for Complex {
    type Output = Complex;
    fn add(self, other: Complex) -> Self::Output {
        let re = self.re + other.re;
        let im = self.im + other.im;
        Complex {re: re, im: im}
    }
}
```


```rust
type Real = Integer | Decimal | Rational;
struct Complex {
    re: Real,
    im: Real,
}
$InfixHandler("+")
lambda(Complex lhs, Complex rhs) {
    re = lhs.re + rhs.re;
    im = lhs.im + rhs.im;
    return Complex {re: re, im: im}
}
```