use super::*;
use crate::value::Symbol;
use patricia_tree::PatriciaMap;

pub struct OptimizedModule {
    inner: PatriciaMap<Symbol>,
}

impl OptimizedModule {
    #[inline]
    pub fn get(&self, path: &str) -> Option<&Symbol> {
        self.inner.get(path)
    }
}

impl From<PackageManager> for OptimizedModule {
    fn from(src: PackageManager) -> Self {
        let mut path = vec![];
        let mut map = PatriciaMap::default();
        let root = match src.get_package_name() {
            None => {
                panic!("package name not found")
            }
            Some(s) => s,
        };
        path.push(root);

        for c in src.get_children_modules() {
            // let sub_name = match c.read().ok().and_then(|f| f.name) {
            //     Some(s) => { s }
            //     None => (Default::default()),
            // };
        }

        return Self { inner: map };
    }
}
