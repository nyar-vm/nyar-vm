pub trait Class {}

impl Clone for Box<dyn Class> {
    fn clone(&self) -> Box<dyn Class> {
        self.clone()
    }
}

impl<T> PartialEq<T> for Box<dyn Class> {
    fn eq(&self, _other: &T) -> bool {
        todo!()
    }
}

impl Eq for Box<dyn Class> {}

pub struct NyarClass {
    name: String,
}
