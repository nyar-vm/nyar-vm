// pub trait Dynamic {
//
// }
//
// impl<T: Any + Clone + Send + Sync> Dynamic for T {
//     #[inline(always)]
//     fn as_any(&self) -> &dyn Any {
//         self
//     }
//     #[inline(always)]
//     fn as_any_mut(&mut self) -> &mut dyn Any {
//         self
//     }
//     #[inline(always)]
//     fn as_object(self) -> Arc<dyn Any> {
//         Arc::new(self)
//     }
//     #[inline(always)]
//     fn type_name(&self) -> &'static str {
//         type_name::<T>()
//     }
//     #[inline(always)]
//     fn clone_object(&self) -> Arc<dyn Dynamic> {
//         self.clone()
//     }
// }
//
// impl dyn Dynamic {
//     /// Check if this [`Dynamic`] is a specific type
//     #[inline(always)]
//     #[must_use]
//     pub fn is<T: Any>(&self) -> bool {
//         TypeId::of::<T>() == self.type_id()
//     }
// }
