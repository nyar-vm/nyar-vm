use super::*;

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn encode_instance(&mut self, instance: &WasiInstance) -> std::fmt::Result {
        write!(self, "(import \"{name}\" (instance ${name}", name = instance.module)?;
        self.indent();
        for wasi in instance.resources.values() {
            self.export_resource(wasi)?;
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
    fn export_resource(&mut self, resource: &WasiResource) -> std::fmt::Result {
        let name = encode_kebab(&resource.wasi_name);
        write!(self, "(export {name} (type (sub resource)))")
    }
    fn alias_export_resource(&mut self, module: &WasiModule, resource: &WasiResource, name: &str) -> std::fmt::Result {
        let id = encode_id(name);
        let name = resource.wasi_name.as_str();
        write!(self, "(alias export ${module} \"{name}\" (type {id}))")
    }
    fn export_parameter(&mut self, input: &WasiParameter) -> std::fmt::Result {
        write!(self, "(param {} {}) ", input.wasi_name, input.r#type)
    }
    fn export_return_type(&mut self, output: &WasiType) -> std::fmt::Result {
        write!(self, "(result {})", output)
    }
    fn export_function(&mut self, function: &WasiFunction) -> std::fmt::Result {
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
        self.dedent(1);
        Ok(())
    }
    //     (alias export $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush" (func $output-stream.blocking-write-and-flush))
    fn alias_export_function(&mut self, module: &WasiModule, function: &WasiFunction, name: &str) -> std::fmt::Result {
        let id = encode_id(name);
        let name = function.wasi_name.as_str();
        write!(self, "(alias export ${module} \"{name}\" (func {id}))")
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub(crate) fn alias_outer(&mut self, ty: &WasiType) -> std::fmt::Result {
        let root = encode_id(&self.source.name);
        match ty {
            WasiType::Resource(resource) => {
                let name = encode_id(&resource.name);
                write!(self, "(alias outer {root} {name} (type {name}))")?
            }
            WasiType::Variant(variant) => {
                let name = variant.symbol.to_string();
                let wasi_name = encode_kebab(&name);
                write!(self, "(alias outer {root} {name} (type {name}?))")?;
                write!(self, "(export {name} {wasi_name} (type (eq {name}?)))")?
            }
            _ => {}
        }
        Ok(())
    }
}
