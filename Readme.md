# Rust Algebra
[![Build Status](https://travis-ci.org/nyar-lang/nyar-number.svg?branch=master)](https://travis-ci.org/nyar-lang/nyar-number)

A Mathematica-like numeric system.


## Type promotion

- *Integer*
  - Integer ± Integer -> Integer
  - Integer * Integer -> Integer
  - Integer / Integer -> Rational

- *Rational*
  - Rational ± Integer -> Rational
  - Rational ± Rational -> Rational
  - Rational */ Rational -> Rational
  - Rational / 0 -> panic!()

- *Decimal*
  - Decimal ± Decimal -> Decimal
  - Decimal ± Rational -> Decimal
  - Decimal ± Decimal -> Decimal