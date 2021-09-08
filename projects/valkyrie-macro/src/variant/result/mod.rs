

pub enum ValkyrieResult<T, E> {
    Success(ValkyrieSuccess<T>),
    Failure(ValkyrieFailure<E>),
}

pub struct ValkyrieSuccess<T> {
    pub value: T,
}

pub struct ValkyrieFailure<E> {
    pub error: E,
}