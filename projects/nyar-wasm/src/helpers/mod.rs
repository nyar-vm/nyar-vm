use nyar_error::NyarError;
use std::intrinsics::transmute;
use wasm_encoder::TypeSection;
use wast::token::{Index, Span};

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
pub trait WasmOutput<'a, Item> {
    fn as_wast(&'a self) -> Item;
}

pub struct Id<'a> {
    name: &'a str,
    gen: u32,
    span: Span,
}

impl<'a> Id<'a> {
    pub fn new(name: &'a str, offset: usize) -> wast::token::Id<'a> {
        unsafe {
            let s = Id { name, gen: 0, span: Span::from_offset(offset) };
            transmute::<Id, wast::token::Id>(s)
        }
    }
    pub fn type_id(name: &'a str, offset: usize) -> Option<wast::token::Id<'a>> {
        Some(Self::new(name, offset))
    }

    pub fn type_index(name: &'a str, offset: usize) -> Option<Index> {
        Some(Index::Id(Self::new(name, offset)))
    }
}
