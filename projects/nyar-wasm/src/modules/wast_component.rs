use super::*;
use wast::{
    component::{Component, ComponentKind},
    token::{NameAnnotation, Span},
    Wat,
};

impl ModuleBuilder {
    pub fn build_component(&self) -> Result<Wat, NyarError> {
        Ok(Wat::Component(Component {
            span: Span::from_offset(0),
            id: None,
            name: Some(NameAnnotation { name: "runtime" }),
            kind: ComponentKind::Text(vec![]),
        }))
    }
}
