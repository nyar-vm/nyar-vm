use nyar_types::*;

#[test]
fn display_scalars_and_helpers() {
    assert_eq!(NyarType::Unit.to_string(), "unit");
    assert!(NyarType::Bottom.is_unitish());
    assert!(NyarType::Float64.is_float());
    assert!(NyarType::Integer64 { signed: true }.is_i64_width());
    assert_eq!(NyarType::Array(Box::new(NyarType::Boolean)).to_string(), "[bool]");
}

#[test]
fn named_and_apply_display() {
    let named = NyarType::Named(Identifier::new("Point"));
    assert_eq!(named.to_string(), "Point");
    let applied = NyarType::Apply(Box::new(named), vec![NyarType::Integer32 { signed: true }]);
    assert_eq!(applied.to_string(), "Point<i32>");
}
