use crate::{helpers::TypeReference, WasiModule};

use super::*;

impl Hash for WasiVariantType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
        self.variants.iter().for_each(|v| {
            v.hash(state);
        });
    }
}

impl ComponentSections for WasiVariantType {
    fn wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, ";; variant {}", self.symbol)?;
        w.newline()?;
        write!(w, "(type {} (variant", self.symbol.wasi_id())?;
        w.indent();
        for variant in self.variants.iter() {
            w.newline()?;
            variant.wasi_define(w)?;
        }
        w.dedent(2);
        Ok(())
    }

    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        let root = &w.source.name;
        let id = self.symbol.wasi_id();
        let name = self.symbol.name.as_ref();
        write!(w, "(alias outer ${root} {id} (type {id}?)) ")?;
        write!(w, "(export {id} \"{name}\" (type (eq {id}?)))")
    }

    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        todo!()
    }

    fn canon_lower<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }

    fn wasm_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}

impl ComponentSections for WasiVariantItem {
    fn wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, ";; {}", self.symbol)?;
        w.newline()?;
        write!(w, "(case \"{}\"", self.wasi_name)?;
        if let Some(s) = &self.fields {
            w.write_char(' ')?;
            s.upper_type(w)?
        }
        w.write_char(')')
    }

    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }

    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        todo!()
    }

    fn canon_lower<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }

    fn wasm_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}
