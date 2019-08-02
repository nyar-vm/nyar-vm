pub trait ToNative {
    type Output;
    fn to_native(&self) -> Self::Output;
}
