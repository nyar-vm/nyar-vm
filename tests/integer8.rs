extern crate number;

use number::{Integer8};

#[test]
fn testing_display() {
    let i = Integer8::from(-1i8);
    assert_eq!(format!("{}", i), "-1i8");

    let i = Integer8::from(0i8);
    assert_eq!(format!("{}", i), "0i8");

    let i = Integer8::from(1i8);
    assert_eq!(format!("{}", i), "1i8");
}
