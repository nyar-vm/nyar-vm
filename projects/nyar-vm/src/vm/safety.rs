use crate::vm::value::{Value, ValueTag};

pub fn instance_of(v: &Value, tag: ValueTag) -> bool {
    v.tag() == tag
}
pub fn check_cast(v: &Value, tag: ValueTag) -> bool {
    v.tag() == tag
}
pub fn bounds_check(len: usize, index: usize) -> bool {
    index < len
}
pub fn validate_call_signature(_: usize, _: usize) -> bool {
    true
}
