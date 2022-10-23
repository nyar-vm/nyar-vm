use std::fmt::Write;

use crate::WastEncoder;

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

impl VariantType {
    pub(crate) fn write_wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, ";; variant {}", self.symbol)?;
        w.newline()?;
        write!(w, "(type {} (variant", self.symbol.wasi_id())?;
        w.indent();
        for (i, variant) in self.variants.values().enumerate() {
            if i != 0 {
                w.newline()?
            }
            variant.write_wasi_define(w)?
        }
        w.dedent(2);
        w.newline()
    }
}

impl VariantItem {
    pub(crate) fn write_wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, ";; {}", self.symbol)?;
        w.newline()?;
        write!(w, "(case \"{}\"", self.wasi_name)?;
        if let Some(s) = &self.fields {
            w.write_char(' ')?;
            s.write_wasi_reference(w)?
        }

        w.write_char(')')
    }
}
