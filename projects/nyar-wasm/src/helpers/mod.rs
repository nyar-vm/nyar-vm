use nyar_error::NyarError;
use wasm_encoder::TypeSection;

pub trait WasmBuilder<Item> {
    type Store = ();

    fn build(&self, store: &Self::Store) -> Result<Item, NyarError>;
}

pub trait WasmDefineType {
    fn emit_type(&self, m: &mut TypeSection) -> Result<(), NyarError>;
}

pub trait WasmEmitter {
    type Receiver = ();
    type Store = ();

    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError>;
}
