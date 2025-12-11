use crate::bytecode::format::NyarcModule;

pub trait AotBackend {
    type Error;
    fn compile(&self, module: &NyarcModule) -> Result<Vec<u8>, Self::Error>;
}
