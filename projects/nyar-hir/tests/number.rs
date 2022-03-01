use std::str::FromStr;
use valkyrie_errors::{third_party::IBig, ValkyrieResult};

#[test]
fn parse_number() -> ValkyrieResult {
    assert_eq!(IBig::from_str("0")?, IBig::from(0));
    assert_eq!(IBig::from_str("1")?, IBig::from(1));
    Ok(())
}
