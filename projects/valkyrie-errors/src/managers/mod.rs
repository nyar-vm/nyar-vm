use std::{
    collections::BTreeMap,
    fmt::{Debug, Display, Formatter},
    fs::read_to_string,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{FileID, ValkyrieResult};
use ariadne::{Cache, Label, Source};
use serde::{Deserialize, Serialize};
use url::Url;

pub mod list;

pub struct TextManager {
    // workspace root
    root: PathBuf,
    max_id: FileID,
    text_map: BTreeMap<FileID, TextItem>,
}

pub struct TextItem {
    path: String,
    source: Arc<Source>,
}

impl Debug for TextManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let files: Vec<&str> = self.text_map.values().map(|v| v.path.as_str()).collect();
        f.debug_struct("TextManager").field("root", &self.root.display()).field("files", &files).finish()
    }
}

impl TextManager {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self { root: root.as_ref().canonicalize().unwrap(), max_id: 0, text_map: BTreeMap::default() }
    }
    pub fn resolve_file(&self, relative_path: &str) -> PathBuf {
        self.root.join(&relative_path)
    }
    pub fn add_file(&mut self, relative_path: &str) -> ValkyrieResult<FileID> {
        let text = read_to_string(&self.resolve_file(relative_path))?;
        Ok(self.add_text(relative_path, text))
    }
    pub fn add_text(&mut self, file: impl Into<String>, text: impl Into<String>) -> FileID {
        let id = self.max_id;
        self.max_id += 1;
        self.text_map.insert(id, TextItem { path: file.into(), source: Arc::new(Source::from(text.into())) });
        id
    }
    // no file and empty file is eqv
    pub fn get_text(&self, id: FileID) -> String {
        match self.text_map.get(&id) {
            Some(s) => s.source.chars().collect(),
            None => String::new(),
        }
    }
}

impl Cache<FileID> for TextManager {
    fn fetch(&mut self, id: &FileID) -> Result<&Source, Box<dyn Debug + '_>> {
        match self.text_map.get(id) {
            Some(s) => Ok(s.source.as_ref()),
            None => Err(Box::new(format!("FileID `{}` not found", id))),
        }
    }

    fn display<'a>(&self, id: &'a FileID) -> Option<Box<dyn Display + 'a>> {
        let item = self.text_map.get(id)?;
        let url = Url::from_file_path(&self.root.join(&item.path)).ok()?;
        Some(Box::new(url))
    }
}
