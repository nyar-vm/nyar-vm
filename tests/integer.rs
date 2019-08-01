extern crate number;

use number::{Integer, Rational};

#[test]
fn testing_display() {
    let i = Integer::from(-1);
    assert_eq!(format!("{}", i), "-1");

    let i = Integer::from(0);
    assert_eq!(format!("{}", i), "0");

    let i = Integer::from(1);
    assert_eq!(format!("{}", i), "1");
}

#[cfg(test)]
mod testing_covert {
    use super::*;

    #[test]
    fn from_hex() {
        let lhs = Integer::from_x("FF");
        let rhs = Integer::from(255);
        assert_eq!(lhs, rhs)
    }

    #[test]
    fn from_oct() {
        let lhs = Integer::from_o("77");
        let rhs = Integer::from(63);
        assert_eq!(lhs, rhs)
    }

    #[test]
    fn from_bin() {
        let lhs = Integer::from_b("11");
        let rhs = Integer::from(3);
        assert_eq!(lhs, rhs)
    }

    #[test]
    fn from_str() {
        let lhs = Integer::from("99");
        let rhs = Integer::from(99);
        assert_eq!(lhs, rhs)
    }
}

#[cfg(test)]
mod testing_operator {
    use super::*;

    #[test]
    fn add() {
        let lhs = Integer::from(2);
        let rhs = Integer::from(3);
        assert_eq!(lhs + rhs, 5)
    }

    #[test]
    fn sub() {
        let lhs = Integer::from(2);
        let rhs = Integer::from(3);
        assert_eq!(lhs.clone() - rhs.clone(), Integer::from("-1"));
        assert_eq!(rhs.clone() - lhs.clone(), Integer::from("+1"));
    }

    #[test]
    fn multiply() {
        let lhs = Integer::from(2);
        let rhs = Integer::from(3);
        assert_eq!(lhs * rhs, 6);
    }

    #[test]
    fn rem() {
        let lhs = Integer::from(2);
        let rhs = Integer::from(5);
        assert_eq!(lhs.clone() % rhs.clone(), 2);
        assert_eq!(rhs.clone() % lhs.clone(), 1);
    }
}
