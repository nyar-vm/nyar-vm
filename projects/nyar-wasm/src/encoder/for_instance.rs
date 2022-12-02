use crate::WasiModule;

use super::*;

impl ComponentSections for CanonicalImport {
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

    fn alias_outer<W: Write>(&self, _: &mut WastEncoder<W>) -> std::fmt::Result {
        unreachable!()
    }

    fn alias_export<W: Write>(&self, _: &mut WastEncoder<W>, _: &WasiModule) -> std::fmt::Result {
        unreachable!()
    }
    fn canon_lower<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::MockMemory => {}
            Self::Instance(v) => {
                for x in v.functions.values() {
                    w.newline()?;
                    x.canon_lower(w)?;
                }
            }
            Self::Type(_) => {}
        }
        Ok(())
    }
    fn wasm_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::MockMemory => {}
            Self::Instance(v) => {
                for x in v.functions.values() {
                    w.newline()?;
                    x.wasm_define(w)?;
                }
            }
            Self::Type(t) => t.wasm_define(w)?,
        }
        Ok(())
    }
}
