use std::io::Result;
use std::path::Ancestors;

mod path;

mod file;

mod directory;

pub struct ValkyrieDirectory {
    _wrap: std::fs::PathBuf,
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

impl ValkyrieFile {
    pub fn open(path: &ValkyriePath) -> Result<ValkyrieFile> {
        let file = std::fs::File::open(path._wrap.clone())?;
        Ok(ValkyrieFile { _wrap: file })
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        let read = self._wrap.read(buffer)?;
        Ok(read)
    }

    pub fn write(&mut self, buffer: &[u8]) -> Result<usize> {
        let written = self._wrap.write(buffer)?;
        Ok(written)
    }

    pub fn close(&mut self) -> Result<()> {
        self._wrap.flush()?;
        Ok(())
    }
}