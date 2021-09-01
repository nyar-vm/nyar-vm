use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{routing::post, Extension, Router};
use tokio::sync::Mutex;
use tower::ServiceBuilder;

use nyar_error::NyarResult;

use crate::local::document::create_user;

mod document;

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
        let service = ServiceBuilder::new()
            // .layer(TraceLayer::new_for_http())
            .layer(Extension(state));
        let router = Router::new() //
            .route("/document", post(create_user))
            .layer(service);
        Ok(router)
    }
    pub fn cache_path(&self) -> PathBuf {
        self.workspace.join("target/language-server/")
    }
}
