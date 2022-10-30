use crate::helpers::TypeReference;

use super::*;

impl Hash for VariantType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
        self.wasi_name.hash(state);
        self.variants.iter().for_each(|v| {
            v.hash(state);
        });
    }
}

impl AliasOuter for VariantType {
    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.newline()?;
        let root = &w.source.name;
        let id = self.symbol.wasi_id();
        let name = self.wasi_name.as_str();
        write!(w, "(alias outer ${root} {id} (type {id}?))")?;
        write!(w, "(export {id} \"{name}\" (type (eq {id}?)))")
    }
}

impl ComponentDefine for VariantType {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, ";; variant {}", self.symbol)?;
        w.newline()?;
        write!(w, "(type {} (variant", self.symbol.wasi_id())?;
        w.indent();
        for variant in self.variants.values() {
            w.newline()?;
            variant.component_define(w)?;
        }
        w.dedent(2);
        Ok(())
    }
}

impl ComponentDefine for VariantItem {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, ";; {}", self.symbol)?;
        w.newline()?;
        write!(w, "(case \"{}\"", self.wasi_name)?;
        if let Some(s) = &self.fields {
            w.write_char(' ')?;
            s.upper_type(w)?
        }
        w.write_char(')')
    }
}
