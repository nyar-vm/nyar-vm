use crate::bytecode::format::NyarcModule;

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
