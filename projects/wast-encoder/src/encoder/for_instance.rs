use crate::{wasi_module::WasiModule, WasiInstance, WasiResource, WasiType};

use super::*;

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn encode_instance(&mut self, instance: &WasiInstance) -> std::fmt::Result {
        write!(self, "(import \"{name}\" (instance ${name}", name = instance.module)?;
        self.indent(true);
        for (id, wasi) in instance.resources.values().enumerate() {
            if id != 0 {
                self.newline()?
            }
            self.export_resource(wasi)?;
        }
        self.dedent(true);
        write!(self, "))")?;
        self.newline()?;
        for (name, wasi) in &instance.resources {
            self.alias_export_resource(&instance.module, wasi, name)?;
            self.newline()?
        }
        Ok(())
    }
    fn export_resource(&mut self, resource: &WasiResource) -> std::fmt::Result {
        let name = self.encode_kebab(&resource.wasi_name);
        write!(self, "(export {name} (type (sub resource)))")
    }
    fn alias_export_resource(&mut self, module: &WasiModule, resource: &WasiResource, name: &str) -> std::fmt::Result {
        let id = self.encode_id(name);
        let name = self.encode_kebab(&resource.wasi_name);
        write!(self, "(alias export ${module} {name} (type {id}))")
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub(crate) fn alias_outer(&mut self, ty: &WasiType) -> std::fmt::Result {
        let root = self.encode_id(&self.source.name);
        match ty {
            WasiType::Resource(resource) => {
                let name = self.encode_id(&resource.name);
                write!(self, "(alias outer {root} {name} (type {name}))")?
            }
            WasiType::Variant(variant) => {
                let name = self.encode_id(&variant.name);
                let wasi_name = self.encode_kebab(&name);
                write!(self, "(alias outer {root} {name} (type {name}?))")?;
                write!(self, "(export {name} {wasi_name} (type (eq {name}?)))")?
            }
            _ => {}
        }
        Ok(())
    }
}
