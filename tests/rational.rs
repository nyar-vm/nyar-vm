extern crate number;

use number::{Integer, Rational};

#[test]
fn testing_display() {
    let r = Rational::new("-2", "+3");
    assert_eq!(format!("{}", r), "-2/3".to_string());

    let r = Rational::new("+2", "-3");
    assert_eq!(format!("{}", r), "-2/3".to_string());

    let r = Rational::new("4", "2");
    assert_eq!(format!("{}", r), "2".to_string());
}

#[cfg(test)]
mod testing_covert {
    use super::*;
    use num::BigInt;

    #[test]
    fn from_string() {
        let r = Rational::new("4", "2");
        assert_eq!(r.numerator(), 2);
        assert_eq!(r.denominator(), 1);
    }

    #[test]
    fn from_integer() {
        let r = Rational::from(2);
        assert_eq!(r.numerator(), 2);
        assert_eq!(r.denominator(), 1);
    }

    #[test]
    fn from_float() {
        let r = Rational::from_float(std::f64::consts::PI);
        assert_eq!(r.numerator(), Integer::from("884279719003555"));
        assert_eq!(r.denominator(), Integer::from("281474976710656"));
    }
}

#[cfg(test)]
mod testing_operator {
    use super::*;

    #[test]
    fn eq() {
        let a = Rational::new("-8", "+6");
        let b = Rational::new("+4", "-3");
        assert_eq!(a, b);
    }

    #[test]
    fn add() {
        let a = Rational::new("8", "6");
        let b = Rational::new("4", "2");
        let c = Integer::from(2);
        assert_eq!(a.clone() + b, Rational::new("10", "3"));
        assert_eq!(a.clone() + c, Rational::new("10", "3"));
    }
}
