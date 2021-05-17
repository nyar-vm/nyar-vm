use crate::Value;
use bigdecimal::BigDecimal;
use num::Num;

pub fn parse_number(s: &str) -> Option<Value> {
    match BigDecimal::from_str_radix(s, 10) {
        Ok(n) => {
            if n.is_integer() {
                Some(Value::Integer(n.as_bigint_and_exponent().0))
            }
            else {
                Some(Value::Decimal(n))
            }
        }
        Err(_) => None,
    }
}
