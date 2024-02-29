use crate::{
    encoder::WastEncoder,
    helpers::{TypeDefinition, TypeReference},
};

use super::*;

impl TypeDefinition for WasiArrayType {
    fn upper_type_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }

    fn lower_type_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}

impl TypeReference for WasiArrayType {
    fn upper_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(list ")?;
        self.r#type.upper_type(w)?;
        w.write_str(")")
    }

    fn lower_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(list ")?;
        self.r#type.lower_type(w)?;
        w.write_str(")")
    }

    fn lower_type_inner<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(array ")?;
        self.r#type.lower_type_inner(w)?;
        w.write_str(")")
    }
}
