use std::intrinsics::transmute;
use wast::{
    core::Instruction,
    token::{Index, Span},
};

pub trait WasmOutput<'a, Item> {
    fn as_wast(&'a self) -> Item;
}

pub trait WasmInstruction {
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i;
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
    pub fn type_id(name: &'a str) -> Option<wast::token::Id<'a>> {
        Some(Self::new(name, 0))
    }

    pub fn type_index(name: &'a str) -> Option<Index> {
        Some(Index::Id(Self::new(name, 0)))
    }
}
