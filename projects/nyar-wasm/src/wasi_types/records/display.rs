use super::*;
use crate::helpers::DependenciesTrace;

impl ComponentDefine for WasiRecordType {
    fn wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, ";; record {}", self.symbol)?;
        w.newline()?;
        write!(w, "(type {} (record", self.symbol.wasi_id())?;
        w.indent();
        for field in self.fields.values() {
            w.newline()?;
            field.wasi_define(w)?;
        }
        w.dedent(2);
        Ok(())
        // (field "x" float32)
    }

    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.newline()?;
        let root = &w.source.name;
        let id = self.symbol.wasi_id();
        let name = self.wasi_name.as_str();
        write!(w, "(alias outer ${root} {id} (type {id}?)) ")?;
        write!(w, "(export {id} \"{name}\" (type (eq {id}?)))")
    }

    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        todo!()
    }
}

impl ComponentDefine for WasiRecordField {
    fn wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(field \"{}\" ", self.wasi_name)?;
        self.r#type.upper_type(w)?;
        write!(w, ")")
    }

    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }

    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        todo!()
    }
}
impl DependenciesTrace for WasiRecordType {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::Record(self.clone()));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.fields.iter().for_each(|(_, v)| v.collect_wasi_types(dict, collected));
    }
}

impl DependenciesTrace for WasiRecordField {
    fn define_language_types(&self, _: &mut DependentGraph) {
        unreachable!()
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.r#type.collect_wasi_types(dict, collected)
    }
}

impl TypeDefinition for WasiRecordType {
    fn lower_type_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}
