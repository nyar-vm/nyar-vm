use super::*;
use crate::helpers::{ComponentSections, TypeReferenceOutput};

impl Display for WasiFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.symbol)?;
        for (i, input) in self.inputs.iter().enumerate() {
            if i != 0 {
                f.write_str(", ")?
            }
            match input.name.as_ref().eq("self") {
                true => f.write_str("self")?,
                false => write!(f, "{}: {:#}", input.name, input.r#type)?,
            }
        }
        f.write_char(')')
    }
}

impl ComponentSections for WasiFunction {
    fn wasi_define<W: Write>(&self, _: &mut WastEncoder<W>) -> std::fmt::Result {
        match &self.body {
            WasiFunctionBody::External { .. } if cfg!(debug_assertions) => {
                panic!("Imported functions cannot be defined using independent wasi: `{}`", self.symbol)
            }
            _ => Ok(()),
        }
    }

    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match &self.body {
            WasiFunctionBody::External { wasi_name, .. } => {
                write!(w, "(export \"{wasi_name}\" (func")?;
            }
            WasiFunctionBody::Native { .. } => {}
        }
        w.indent();
        for input in self.inputs.iter() {
            w.newline()?;
            input.upper_input(w)?;
        }
        for output in self.output.iter() {
            w.newline()?;
            output.r#type.upper_output(w)?;
        }
        w.dedent(2);
        Ok(())
    }

    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        let id = self.symbol.wasi_id();
        match &self.body {
            WasiFunctionBody::External { wasi_name, .. } => write!(w, "(alias export ${module} \"{wasi_name}\" (func {id}))")?,
            WasiFunctionBody::Native { .. } => {}
        }
        Ok(())
    }
    fn canon_lower<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match &self.body {
            WasiFunctionBody::External { wasi_module, wasi_name } => {
                write!(w, "(core func {} (canon lower", self.symbol.wasi_id())?;
                w.indent();
                w.newline()?;
                write!(w, "(func ${} \"{}\")", wasi_module, wasi_name)?;
                if self.need_heap() {
                    w.newline()?;
                    write!(w, "(memory $memory \"memory\")")?;
                    write!(w, "(realloc (func $memory \"realloc\"))")?;
                }
                if let Some(encode) = self.need_encoding() {
                    w.newline()?;
                    write!(w, "string-encoding={encode}")?;
                }
                w.dedent(2);
            }
            WasiFunctionBody::Native { .. } => {}
        }
        Ok(())
    }

    fn wasm_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match &self.body {
            WasiFunctionBody::External { wasi_module, wasi_name } => {
                write!(w, "(import \"{}\" \"{}\" (func {}", wasi_module, wasi_name, self.symbol.wasi_id())?;
            }
            WasiFunctionBody::Native { .. } => {
                write!(w, "(func {}", self.symbol.wasi_id())?;
            }
        }
        w.indent();
        for input in &self.inputs {
            w.newline()?;
            input.lower_input(w)?;
        }
        for output in &self.output {
            w.newline()?;
            output.lower_input(w)?;
        }
        match &self.body {
            WasiFunctionBody::External { .. } => w.dedent(2),
            WasiFunctionBody::Native { .. } => w.dedent(1),
        }
        Ok(())
    }
}
