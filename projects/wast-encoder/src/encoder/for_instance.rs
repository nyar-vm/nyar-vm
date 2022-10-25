use crate::{Identifier, WasiModule};

use super::*;

impl ComponentDefine for CanonicalImport {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Type(v) => v.component_define(w),
            Self::Instance(v) => v.component_define(w),
        }
    }
}

impl ComponentDefine for WasiInstance {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(import \"{name}\" (instance ${name}", name = self.module)?;
        w.indent();
        for wasi in self.resources.values() {
            wasi.write_wasi_define(w)?;
            w.newline()?;
        }
        for wasi in self.functions.values() {
            w.export_function(wasi)?;
            w.newline()?
        }
        w.dedent(2);
        w.newline()?;
        for (name, wasi) in &self.resources {
            w.alias_export_resource(&self.module, wasi, name)?;
            w.newline()?
        }
        for (name, wasi) in &self.functions {
            w.alias_export_function(&self.module, wasi, name)?;
            w.newline()?
        }
        Ok(())
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    fn alias_export_resource(&mut self, module: &WasiModule, resource: &WasiResource, name: &Identifier) -> std::fmt::Result {
        let id = name.wasi_id();
        let name = resource.wasi_name.as_str();
        write!(self, "(alias export ${module} \"{name}\" (type {id}))")
    }
    fn export_parameter(&mut self, input: &WasiParameter) -> std::fmt::Result {
        write!(self, "(param \"{}\" ", input.wasi_name)?;
        input.r#type.write_wasi_reference(self)?;
        self.write_str(") ")
    }
    fn export_function(&mut self, function: &ExternalFunction) -> std::fmt::Result {
        let name = function.wasi_name.as_str();
        write!(self, "(export \"{name}\" (func")?;
        self.indent();
        for input in &function.inputs {
            self.export_parameter(input)?;
            self.newline()?
        }
        for output in function.output.iter() {
            output.write_wasi_result(self)?;
        }
        self.dedent(2);
        Ok(())
    }
    //     (alias export $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush" (func $output-stream.blocking-write-and-flush))
    fn alias_export_function(
        &mut self,
        module: &WasiModule,
        function: &ExternalFunction,
        name: &Identifier,
    ) -> std::fmt::Result {
        let id = name.wasi_id();
        let name = function.wasi_name.as_str();
        write!(self, "(alias export ${module} \"{name}\" (func {id}))")
    }
}
