use std::rc::Rc;

pub struct Identifier {
    path: Vec<Rc<str>>,
    span: FileSpan,
}

impl FromIterator<Rc<str>> for Identifier {
    fn from_iter<T: IntoIterator<Item = Rc<str>>>(iter: T) -> Self {
        Self { path: iter.into_iter().collect(), span: Default::default() }
    }
}
