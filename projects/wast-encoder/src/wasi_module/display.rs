use super::*;

//     (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
//         (export "error" (type (sub resource)))
//     ))
//     (alias export $wasi:io/error@0.2.0 "error" (type $io-error))
impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn encode_instance(&mut self, wasi: WasiInstance) -> std::fmt::Result {
        write!(self, "(import \"{name}\" (instance ${name}", name = wasi.module)?;
        self.indent();
        self.export_resource("error")?;
        self.dedent();
        self.newline()?;
        write!(self, "))")?;
        self.alias_export_type(&wasi.module, "error", "std::io::IoError")?;
        Ok(())
    }
    fn export_resource(&mut self, wasi_name: &str) -> std::fmt::Result {
        write!(self, "(export \"{wasi_name}\" (type (sub resource)))")?;
        self.newline()
    }

    fn alias_export_type(&mut self, module: &WasiModule, wasi_name: &str, name: &str) -> std::fmt::Result {
        let id = self.encode_id(name);
        write!(self, "(alias export ${module} \"{wasi_name}\" (type {id}))")?;
        self.newline()
    }
}

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
