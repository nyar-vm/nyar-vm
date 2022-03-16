use indexmap::{map::Iter, IndexMap};
use nyar_error::NyarError;

pub trait WasmBuilder<Item> {
    type Store = ();

    fn build(&self, store: &Self::Store) -> Result<Item, NyarError>;
}
