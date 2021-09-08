use std::path::{Ancestors, PathBuf};

pub struct ValkyrieDirectory {
    _wrap: PathBuf,
}


impl ValkyrieDirectory {
    // parent returns the parent directory of the current directory.
    pub fn parent(&self) -> Option<ValkyrieDirectory> {
        self._wrap.parent().map(|p| ValkyrieDirectory { _wrap: p.to_path_buf() })
    }
    pub fn ancestors(&self) -> Ancestors<'_> {
        self._wrap.ancestors()
    }

    pub fn childrens(&self, depth: u32) -> Ancestors<'_> {
        self._wrap.ancestors()
    }
    pub fn files(&self) -> Ancestors<'_> {
        self._wrap.ancestors()
    }
    pub fn directories(&self) -> Ancestors<'_> {
        self._wrap.ancestors()
    }

    pub fn descendants(&self) -> Ancestors<'_> {
        self._wrap.ancestors()
    }
}
