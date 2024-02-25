use crate::{Identifier, WasiModule};

use super::*;

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn encode_instance(&mut self, instance: &WasiInstance) -> std::fmt::Result {
        write!(self, "(import \"{name}\" (instance ${name}", name = instance.module)?;
        self.indent();
        for wasi in instance.resources.values() {
            wasi.write_wasi_define(self)?;
            self.newline()?;
        }
        for (id, wasi) in instance.functions.values().enumerate() {
            if id != 0 {
                self.newline()?
            }
            self.export_function(wasi)?;
        }
        self.dedent(2);
        self.newline()?;
        for (name, wasi) in &instance.resources {
            self.alias_export_resource(&instance.module, wasi, name)?;
            self.newline()?
        }
        for (name, wasi) in &instance.functions {
            self.alias_export_function(&instance.module, wasi, name)?;
            self.newline()?
        }
        Ok(())
    }
    fn alias_export_resource(&mut self, module: &WasiModule, resource: &WasiResource, name: &Identifier) -> std::fmt::Result {
        let id = name.wasi_id();
        let name = resource.wasi_name.as_str();
        write!(self, "(alias export ${module} \"{name}\" (type {id}))")
    }
    fn export_parameter(&mut self, input: &WasiParameter) -> std::fmt::Result {
        write!(self, "(param {} ", input.wasi_name)?;
        input.r#type.write_wasi_reference(self)?;
        self.write_str(") ")
    }
    fn export_return_type(&mut self, output: &WasiType) -> std::fmt::Result {
        write!(self, "(result ")?;
        output.write_wasi_reference(self)?;
        self.write_str(") ")
    }
    fn export_function(&mut self, function: &ExternalFunction) -> std::fmt::Result {
        let name = function.wasi_name.as_str();
        write!(self, "(export \"{name}\"")?;
        self.indent();
        write!(self, "(func ")?;
        for input in &function.inputs {
            self.export_parameter(input)?
        }
        for output in function.output.iter() {
            self.export_return_type(output)?
        }
        self.write_str(")")?;
        self.dedent(1);
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

impl<'a, W: Write> WastEncoder<'a, W> {
    pub(crate) fn alias_outer(&mut self, ty: &WasiType) -> std::fmt::Result {
        let root = encode_id(&self.source.name);
        match ty {
            WasiType::Resource(resource) => {
                let id = resource.symbol.wasi_id();
                let name = resource.wasi_name.as_str();
                write!(self, "(alias outer {root} \"{name}\" (type {id}))")?
            }
            WasiType::Variant(variant) => {
                let id = variant.symbol.wasi_id();
                let name = variant.wasi_name.as_str();
                write!(self, "(alias outer {root} {id} (type {id}?))")?;
                write!(self, "(export {id} \"{name}\" (type (eq {id}?)))")?
            }
            _ => {}
        }
        Ok(())
    }
}
