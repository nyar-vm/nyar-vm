use super::*;

impl Display for WasiModule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = &self.package {
            write!(f, "{}/", s)?
        }
        write!(f, "{}", self.name)?;
        if let Some(s) = &self.version {
            write!(f, "@{}", s)?
        }
        Ok(())
    }
}

impl Display for WasiPublisher {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.organization, self.project)
    }
}

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
        for (language, wasi) in &instance.resources {
            self.alias_export_type(&instance.module, wasi, language)?;
            self.newline()?
        }
        Ok(())
    }
    fn export_resource(&mut self, wasi_name: &str) -> std::fmt::Result {
        write!(self, "(export \"{wasi_name}\" (type (sub resource)))")
    }
    fn alias_export_type(&mut self, module: &WasiModule, wasi_name: &str, name: &str) -> std::fmt::Result {
        let id = self.encode_id(name);
        write!(self, "(alias export ${module} \"{wasi_name}\" (type {id}))")
    }
}
