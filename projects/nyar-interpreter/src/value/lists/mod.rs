use std::{convert::TryFrom, num::TryFromIntError};

use bigdecimal::num_bigint::{Sign, TryFromBigIntError};
use num::Signed;

use nyar_error::NyarError;

use super::*;

pub struct Vector {
    typing: SharedValue,
    values: BTreeMap<u64, SharedValue>,
}

pub struct Array {
    typing: SharedValue,
    values: Vec<SharedValue>,
}

impl Value {
    pub fn as_index(&self) -> NyarResult<(u64, bool)> {
        match self {
            Value::Integer(v) => {
                let (s, i) = v.into_parts();
                let index = match u64::try_from(i) {
                    Ok(o) => o,
                    Err(_) => {
                        let msg = format!("index `{}` is overflow", v);
                        return Err(NyarError::invalid_index(msg, Default::default()));
                    }
                };
                let out = matches!(s, Sign::NoSign | Sign::Plus);
                Ok((index, out))
            }
            _ => {
                let msg = format!("type `{}` is not a valid index type", self.as_type());
                Err(NyarError::invalid_index(msg, Default::default()))
            }
        }
    }
}

impl Vector {
    pub fn get_index(n: SharedValue) -> NyarResult<Self> {
        let a = n.read()?.as_index()?;
    }
}
