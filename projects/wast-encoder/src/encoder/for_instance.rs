use crate::helpers::{TypeReferenceInput, TypeReferenceOutput};

use super::*;

impl ComponentDefine for CanonicalImport {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
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
            Self::Type(v) => v.component_define(w),
            Self::Instance(v) => v.component_define(w),
        }
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub(crate) fn export_function(&mut self, function: &WasiExternalFunction) -> std::fmt::Result {
        let name = function.wasi_name.as_str();
        write!(self, "(export \"{name}\" (func")?;
        self.indent();
        for input in function.inputs.iter() {
            self.newline()?;
            input.upper_input(self)?;
        }
        for output in function.output.iter() {
            self.newline()?;
            output.upper_output(self)?;
        }
        self.dedent(2);
        Ok(())
    }
}
