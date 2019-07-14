extern crate number;
use number::Decimal;

#[cfg(test)]
mod testing_covert {
    use super::*;

    #[test]
    fn from_str() {
        let n = Decimal::from("12345");
        assert_eq!(format!("{}", n), String::from("12345"));
        let n = Decimal::from("12345.67");
        assert_eq!(format!("{}", n), String::from("12345.67"));
        let n = Decimal::from("12345.670");
        assert_eq!(format!("{}", n), String::from("12345.670"));
    }

    #[test]
    fn from_acc() {
        let n = Decimal::new("12345.670", "7");
        assert_eq!(format!("{}", n), String::from("12345.6700000"));
        let n = Decimal::new("12345.670", "0");
        assert_eq!(format!("{}", n), String::from("12345"));
        let n = Decimal::new("12345.670", "-3");
        assert_eq!(format!("{}", n), String::from("12000"));
    }
}

#[cfg(test)]
mod testing_operator {
    use super::*;

    #[test]
    fn add() {
        let a = Decimal::from("3.141592653");
        let b = Decimal::from("2.718281828");
        let r = Decimal::from("5.859874481");
        assert_eq!(a + b, r)
    }

    #[test]
    fn sub() {
        let a = Decimal::from("3.141592653");
        let b = Decimal::from("2.718281828");
        let r = Decimal::from("0.423310825");
        assert_eq!(a - b, r)
    }
}
