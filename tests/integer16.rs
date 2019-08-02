extern crate number;

use number::{Integer16, Integer8};

#[test]
fn testing_up_cast() {
    let i = Integer8::from(0i8);
    assert_eq!(format!("{}", i), "0i8");

    let i = Integer8::from("0");
    assert_eq!(format!("{}", i), "0i8");

    let i = Integer8::from(Integer8::from(0));
    assert_eq!(format!("{}", i), "0i8");
}
