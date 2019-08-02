


Number -



## Integer

### Up

Integer ± Integer -> Integer
Rational ± Integer -> Rational
Decimal ± Integer -> Decimal
Decimal ± Decimal -> Decimal







### Nyar

```ts
type Real = Integer | Decimal | Rational;
class Complex {
    var re <- Real : 0;
    var im <- Real : 0;

    $InfixHandler("+")
    lambda(self, Complex other) {
        re = self.re + other.re;
        im = self.im + other.im;
        return Complex(re: re, im: im)
    }
}
```

### Rust

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