mod parser;

pub use bigdecimal::BigDecimal;
pub use num::{BigInt, Num};
pub use parser::parse_number;

pub type OrderedMap<K, V> = std::collections::HashMap<K, V>;

// pub type OrderedMap<K, V> = indexmap::IndexMap<K,V>;
