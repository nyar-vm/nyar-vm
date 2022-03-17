use nyar_error::NyarError;

pub trait WasmBuilder<Item> {
    type Store = ();

    fn build(&self, store: &Self::Store) -> Result<Item, NyarError>;
}

pub trait WasmEmitter {
    type Receiver = ();
    type Store = ();

    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError>;
}
