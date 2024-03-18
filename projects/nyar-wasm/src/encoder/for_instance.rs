use crate::{
    helpers::{TypeReferenceInput, TypeReferenceOutput},
    wasi_types::functions::WasiFunctionBody,
    WasiModule,
};

use super::*;

impl ComponentDefine for CanonicalImport {
    fn wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::MockMemory => w.write_str(
                r#"(core module $MockMemory
        (func $realloc (export "realloc") (param i32 i32 i32 i32) (result i32)
            (i32.const 0)
        )
        (memory $memory (export "memory") 15)
    )
    (core instance $memory (instantiate $MockMemory))"#,
            ),
            Self::Type(v) => v.wasi_define(w),
            Self::Instance(v) => {
                w.newline()?;
                v.wasi_define(w)
            }
        }
    }

    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }

    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        todo!()
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub(crate) fn export_function(&mut self, function: &WasiFunction) -> std::fmt::Result {
        match &function.body {
            WasiFunctionBody::External { wasi_name, .. } => {
                write!(self, "(export \"{wasi_name}\" (func")?;
            }
            WasiFunctionBody::Normal { .. } => {}
        }

        self.indent();
        for input in function.inputs.iter() {
            self.newline()?;
            input.upper_input(self)?;
        }
        for output in function.output.iter() {
            self.newline()?;
            output.r#type.upper_output(self)?;
        }
        self.dedent(2);
        Ok(())
    }
}
