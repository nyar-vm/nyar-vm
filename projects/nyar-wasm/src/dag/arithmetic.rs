use super::*;
use crate::helpers::DependenciesTrace;

impl<T> AddAssign<T> for DependentGraph
where
    T: DependenciesTrace,
{
    fn add_assign(&mut self, rhs: T) {
        rhs.define_language_types(self);
    }
}
