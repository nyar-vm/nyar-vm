use crate::bytecode::format::NyarcModule;

mod jvm_backend;
mod clr_backend;
mod x86_backend;
mod wasm32_backend;

pub trait AotCompiler {
    type Error;
    type Config: serde::de::DeserializeOwned;
    fn compile(&self, module: &NyarcModule) -> Result<Vec<u8>, Self::Error>;
    fn compile_with_config(
        &self,
        module: &NyarcModule,
        config: &Self::Config,
    ) -> Result<Vec<u8>, Self::Error>;
}
