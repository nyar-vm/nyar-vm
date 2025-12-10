#[derive(Clone)]
pub struct FFIDescriptor {
    pub name: String,
}

pub fn ffi_call(_: FFIDescriptor, _: &[u8]) {}
