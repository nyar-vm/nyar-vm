//! widget 渲染合同检查。

use crate::types::{
    Identifier, SourceSpan,
    hir::{HirFunction, HirModule, HirWidget, ValkyrieType as HirType},
};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetErrorKind {
    MissingRenderMethod { widget: Identifier },
    InvalidRenderReturnType { widget: Identifier, found: HirType },
    InvalidStateUpdate { widget: Identifier, field: Identifier },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetError {
    pub kind: WidgetErrorKind,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl WidgetError {
    pub fn missing_render_method(widget: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: WidgetErrorKind::MissingRenderMethod { widget: widget.clone() },
            message: format!("widget {} is missing render method", widget),
            span,
        }
    }

    pub fn invalid_render_return_type(widget: Identifier, found: HirType, span: Option<SourceSpan>) -> Self {
        Self {
            kind: WidgetErrorKind::InvalidRenderReturnType { widget: widget.clone(), found: found.clone() },
            message: format!("widget {} render must return Element, found {:?}", widget, found),
            span,
        }
    }

    pub fn invalid_state_update(widget: Identifier, field: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: WidgetErrorKind::InvalidStateUpdate { widget: widget.clone(), field: field.clone() },
            message: format!("widget {} has invalid state update for {}", widget, field),
            span,
        }
    }
}

impl fmt::Display for WidgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for WidgetError {}

#[derive(Debug, Default)]
pub struct WidgetChecker {
    widgets: BTreeMap<Identifier, HirWidget>,
    errors: Vec<WidgetError>,
    current_widget: Option<Identifier>,
    in_event_handler: bool,
    in_lifecycle_method: bool,
}

impl WidgetChecker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn collect_widgets(&mut self, module: &HirModule) {
        for widget in &module.widgets {
            self.widgets.insert(widget.name.clone(), widget.clone());
        }
        for submodule in &module.submodules {
            self.collect_widgets(submodule);
        }
    }

    pub fn check_module(&mut self, module: &HirModule) -> Vec<WidgetError> {
        self.clear();
        self.collect_widgets(module);
        for widget in module.widgets.iter().chain(module.submodules.iter().flat_map(|module| module.widgets.iter())) {
            self.current_widget = Some(widget.name.clone());
            if let Some(render) = self.find_render_method(widget) {
                if !self.is_element_type(&render.return_type) {
                    self.errors.push(WidgetError::invalid_render_return_type(widget.name.clone(), render.return_type.clone(), None));
                }
            }
            else {
                self.errors.push(WidgetError::missing_render_method(widget.name.clone(), None));
            }
        }
        self.current_widget = None;
        self.errors.clone()
    }

    pub fn is_element_type(&self, ty: &HirType) -> bool {
        matches!(ty, HirType::Named(name) if name == &Identifier::new("Element"))
    }

    pub fn is_event_handler_method(&self, name: &str) -> bool {
        name.starts_with("on") && !matches!(name, "on_mount" | "on_unmount" | "on_update" | "before_update" | "after_update")
    }

    pub fn is_lifecycle_method(&self, name: &str) -> bool {
        matches!(name, "on_mount" | "on_unmount" | "on_update" | "before_update" | "after_update")
    }

    pub fn is_state_field(&self, name: &Identifier) -> bool {
        let text = name.as_str();
        text.starts_with('_') || text.starts_with("state_")
    }

    pub fn is_valid_state_update_context(&self) -> bool {
        self.in_event_handler || self.in_lifecycle_method
    }

    pub fn set_in_event_handler(&mut self, value: bool) {
        self.in_event_handler = value;
    }

    pub fn set_in_lifecycle_method(&mut self, value: bool) {
        self.in_lifecycle_method = value;
    }

    pub fn get_widget_names(&self) -> Vec<Identifier> {
        self.widgets.keys().cloned().collect()
    }

    pub fn widgets(&self) -> &BTreeMap<Identifier, HirWidget> {
        &self.widgets
    }

    pub fn errors(&self) -> &[WidgetError] {
        &self.errors
    }

    pub fn current_widget(&self) -> Option<&Identifier> {
        self.current_widget.as_ref()
    }

    pub fn clear(&mut self) {
        self.widgets.clear();
        self.errors.clear();
        self.current_widget = None;
        self.in_event_handler = false;
        self.in_lifecycle_method = false;
    }

    pub fn find_render_method<'a>(&self, widget: &'a HirWidget) -> Option<&'a HirFunction> {
        widget.methods.iter().find(|method| matches!(method.name.as_str(), "render" | "view"))
    }
}
