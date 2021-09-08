use std::str::FromStr;

use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use valkyrie_parser::{ValkyrieKeyword, ValkyrieOperator};

use crate::utils::ColoredWriter;

mod jetbrain;

pub async fn request_document(
    Json(request): Json<IDocument>,
    // Extension(state): Extension<LanguageState>,
) -> JsonResponse<ODocument> {
    match request.as_typed() {
        Some(s) => s.render_jetbrain(),
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ODocument {
    pub content: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Dependence {
    pub kind: String,
    pub language: String,
    pub namepath: Vec<String>,
}

pub enum TypedDocument {
    Keywords(ValkyrieKeyword),
    Operator(ValkyrieOperator),
    Trait { namepath: Vec<String> },
}

impl IDocument {
    pub fn as_typed(&self) -> Option<TypedDocument> {
        match self.kind.as_str() {
            "keyword" => {
                let first = self.namepath.first()?;
                let token = ValkyrieKeyword::from_str(first).ok()?;
                Some(TypedDocument::Keywords(token))
            }
            "operator" => {
                let first = self.namepath.first()?;
                let token = ValkyrieOperator::from_str(first).ok()?;
                Some(TypedDocument::Operator(token))
            }
            "trait" => Some(TypedDocument::Trait { namepath: self.namepath.clone() }),
            _ => None,
        }
    }
}
