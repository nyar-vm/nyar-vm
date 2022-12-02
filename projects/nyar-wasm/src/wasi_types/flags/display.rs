use super::*;

impl Display for WasiFlags {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl ComponentSections for WasiFlags {
    fn wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(type {} (flags", self.symbol.wasi_id())?;
        w.indent();
        for (index, variant) in self.variants.values().enumerate() {
            w.newline()?;
            write!(w, "\"{}\" ;; {}", variant.wasi_name, index)?;
        }
        w.dedent(2);
        Ok(())
    }

    fn alias_outer<W: Write>(&self, _: &mut WastEncoder<W>) -> std::fmt::Result {
        unreachable!()
    }

    fn alias_export<W: Write>(&self, _: &mut WastEncoder<W>, _: &WasiModule) -> std::fmt::Result {
        unreachable!()
    }

    fn canon_lower<W: Write>(&self, _: &mut WastEncoder<W>) -> std::fmt::Result {
        unreachable!()
    }

    fn wasm_define<W: Write>(&self, _: &mut WastEncoder<W>) -> std::fmt::Result {
        unreachable!()
    }
}
