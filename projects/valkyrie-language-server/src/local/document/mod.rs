use axum::{extract::Query, http::StatusCode, Extension, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::local::LanguageState;

#[derive(Clone, Debug, Deserialize)]
pub struct IDocument {
    pub kind: String,
    pub language: String,
    pub namepath: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ODocument {
    pub content: String,
}

// #[axum_macros::debug_handler]
// pub async fn request_document(
//     Json(request): Json<IDocument>,
//     Extension(state): Extension<LanguageState>,
// ) -> (StatusCode, Json<ODocument>) {
//     let server = state.lock().await;
//     (StatusCode::OK, Json(ODocument { content: format!("{:?}", request) }))
// }
#[derive(Deserialize)]
pub struct CreateUser {
    email: String,
    password: String,
}

pub async fn create_user(Json(payload): Json<CreateUser>) {
    // ...
}
