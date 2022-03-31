use crate::ValkyrieType;

#[track_caller]
pub fn assert_type<T>(value: T, short: &str, long: &str)
where
    T: ValkyrieType + 'static,
{
    let v = value.dynamic_type();
    assert_eq!(format!("{}", v), short);
    assert_eq!(v.get().display_type(true), long);
}
