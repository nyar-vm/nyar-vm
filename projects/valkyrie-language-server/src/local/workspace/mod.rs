use std::str::FromStr;

use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use valkyrie_parser::{ValkyrieKeyword, ValkyrieOperator};

use crate::utils::ColoredWriter;

pub async fn request_document(
    Json(request): Json<DependencyRequest>,
    Extension(state): Extension<LanguageState>,
) -> JsonResponse<Vec<DependencyLibrary>> {
    match request.as_typed() {
        Some(s) => s.render_jetbrain(),
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct DependencyRequest {
    pub content: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DependencyLibrary {
    pub kind: String,
    pub language: String,
    pub namepath: Vec<String>,
}
