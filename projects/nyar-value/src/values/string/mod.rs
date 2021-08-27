use std::fmt::{Display, Formatter};

use convert_case::{Case, Casing};
use smartstring::{LazyCompact, SmartString};

use nyar_macro::sync_value_type;

use crate::NyarValue;

#[derive(Clone, Debug)]
pub struct NyarString {
    inner: SmartString<LazyCompact>,
}

sync_value_type!(NyarString);

impl Display for NyarString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl NyarString {
    pub fn length(&self) -> usize {
        self.inner.len()
    }

    pub fn contains(&self, pattern: NyarValue) -> bool {
        todo!()
    }
    pub fn count(&self, pattern: NyarValue) -> usize {
        todo!()
    }

    pub fn join(&self, other: &str) -> NyarString {
        todo!()
    }

    pub fn size(&self) -> usize {
        self.inner.len()
    }

    pub fn repeat(&self, n: usize) -> String {
        self.inner.repeat(n)
    }

    pub fn pad(&self) {}
    pub fn pad_front(&self) {}
}

impl NyarString {
    pub fn to_uppercase(&self) -> String {
        self.inner.to_case(Case::Upper)
    }
    pub fn to_ascii_uppercase(&self) -> String {
        self.inner.to_ascii_uppercase()
    }
    pub fn to_lowercase(&self) -> String {
        self.inner.to_case(Case::Lower)
    }
    pub fn to_ascii_lowercase(&self) -> String {
        self.inner.to_ascii_lowercase()
    }
    pub fn to_camelcase(&self) -> String {
        self.inner.to_case(Case::Camel)
    }
    pub fn to_pascalcase(&self) -> String {
        self.inner.to_case(Case::UpperCamel)
    }
    pub fn to_snakecase(&self) -> String {
        self.inner.to_case(Case::Snake)
    }
}

impl NyarString {
    pub fn push(&mut self, c: char) {
        self.inner.push(c)
    }
}

impl NyarString {
    pub fn extend(&mut self) {}
}
