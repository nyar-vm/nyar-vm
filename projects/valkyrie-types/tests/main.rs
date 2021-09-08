use num::BigInt;
use std::any::type_name;

use valkyrie_types::testing::assert_type;

#[test]
fn ready() {
    println!("it works!")
}

#[test]
fn test_primitive() {
    let value: usize = 0;
    assert_type(value, "Unsigned64", "std::primitive::Unsigned64");
    let value: f64 = 0.0;
    assert_type(value, "Float64", "std::primitive::Float64");
}

#[test]
fn test_list() {
    let value: Vec<usize> = vec![];
    assert_type(value, "List[u64]", "std::primitive::Option[std::primitive::u64]");
    // let value: Option<usize> = None;
    // assert_type(value, "Option[u64]", "std::primitive::Option[std::primitive::u64]");
    // let value: Option<Option<usize>> = Some(None);
    // assert_type(value, "Option[Option[u64]]", "std::primitive::Option[std::primitive::Option]");
}

#[test]
fn test_option() {
    let value: Option<usize> = Some(0);
    assert_type(value, "Option[u64]", "std::primitive::Option[std::primitive::u64]");
    // let value: Option<usize> = None;
    // assert_type(value, "Option[u64]", "std::primitive::Option[std::primitive::u64]");
    // let value: Option<Option<usize>> = Some(None);
    // assert_type(value, "Option[Option[u64]]", "std::primitive::Option[std::primitive::Option]");
}
