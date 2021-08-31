use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{Extension, Router};
use tokio::sync::Mutex;

use nyar_error::NyarResult;

use crate::App;

pub type LanguageState = Arc<Mutex<LanguageServer>>;

#[derive(Clone)]
pub struct LanguageServer {
    workspace: PathBuf,
    prelude: String,
}

impl LanguageServer {
    pub fn start<P: AsRef<Path>>(path: P) -> NyarResult<Router> {
        let path = path.as_ref().canonicalize()?;
        if !path.is_dir() {}
        if !path.join("fleet.json5").exists() {}
        let server = LanguageServer { workspace: path, prelude: "".to_string() };
        let state = Arc::new(Mutex::new(server));
        Router::new().route("/", get(handler)).layer(AddExtensionLayer::new(state))
    }
}

pub async fn start_local(Extension(state): Extension<LanguageState>) {
    let server = state.lock().await;
}
