use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};

mod jetbrain;

pub async fn request_document(
    Json(request): Json<IDocument>,
    // Extension(state): Extension<LanguageState>,
) -> Result<Json<ODocument>, StatusCode> {
    request.to_typed().render_jetbrain()
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ODocument {
    pub content: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IDocument {
    pub kind: String,
    pub language: String,
    pub namepath: Vec<String>,
}

pub enum TypedDocument {
    Nothing,
    Keywords(String),
}

impl IDocument {
    pub fn to_typed(&self) -> TypedDocument {
        match self.kind.as_str() {
            "keyword" => match self.namepath.first() {
                None => TypedDocument::Nothing,
                Some(s) => TypedDocument::Keywords(s.to_string()),
            },
            _ => TypedDocument::Nothing,
        }
    }
}
