extern crate number;

use number::Float32;

#[test]
fn testing_display() {
    let f = Float32::from(0.00);
    assert_eq!(format!("{}", f), "0.0f32");
}
