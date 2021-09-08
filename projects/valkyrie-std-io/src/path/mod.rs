// #[v::namepath(std::io::Path)]
pub struct ValkyriePath {
    _wrap: std::path::PathBuf,
}

impl ValkyriePath {
    pub fn get_windows_form(&self) -> String {
        self._wrap.to_str().unwrap().to_string()
    }
    pub fn get_url(&self) -> String {
        self._wrap.to_str().unwrap().to_string()
    }
    pub fn get_unix_form(&self) -> String {
        self._wrap.to_str().unwrap().to_string()
    }
}
