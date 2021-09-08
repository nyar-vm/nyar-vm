use std::fmt::Formatter;

use serde::{Deserialize, Deserializer, Serialize};
use serde::de::{MapAccess, Visitor};
use serde_types::OneOrMany;

#[derive(Debug, Clone, Serialize)]
pub struct Workspace {
    pub enabled: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl Workspace {
    pub fn enable(&mut self) {
        self.enabled = true;
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            enabled: false,
            include: vec![],
            exclude: vec![],
        }
    }
}

impl<'de> Deserialize<'de> for Workspace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let mut default = Workspace::default();
        Deserialize::deserialize_in_place(deserializer, &mut default)?;
        return Ok(default);
    }
    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_any(WorkspaceVisitor { body: place })
    }
}

struct WorkspaceVisitor<'de> {
    body: &'de mut Workspace,
}

impl<'de, 'body> Visitor<'de> for WorkspaceVisitor<'body> {
    type Value = ();

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("Except Workspace object")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "include" => {
                    self.body.include = map.next_value::<OneOrMany<String>>()?.unwrap();
                }
                "exclude" => {
                    self.body.exclude = map.next_value::<OneOrMany<String>>()?.unwrap();
                }
                _ => {}
            }
        }
        todo!()
    }
}