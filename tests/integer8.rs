extern crate number;

use number::Integer8;

#[test]
fn testing_display() {
    let i = Integer8::from(0i8);
    assert_eq!(format!("{}", i), "0i8");

    let i = Integer8::from("0");
    assert_eq!(format!("{}", i), "0i8");

    let i = Integer8::from(Integer8::from(0));
    assert_eq!(format!("{}", i), "0i8");
}

#[cfg(test)]
mod testing_operator {
    use super::*;

    #[test]
    fn add() {
        let lhs = Integer8::from(2);
        let rhs = Integer8::from(3);
        assert_eq!(lhs + rhs, 5i8);
        assert_eq!(lhs + rhs, 5)
    }

    #[test]
    fn sub() {
        let lhs = Integer8::from(2);
        let rhs = Integer8::from(3);
        assert_eq!(lhs - rhs, Integer8::from("-1"));
        assert_eq!(rhs - lhs, Integer8::from("+1"));
    }

}
