use super::*;

impl AliasOuter for WasiRecordType {
    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.newline()?;
        let root = &w.source.name;
        let id = self.symbol.wasi_id();
        let name = self.wasi_name.as_str();
        write!(w, "(alias outer ${root} {id} (type {id}?)) ")?;
        write!(w, "(export {id} \"{name}\" (type (eq {id}?)))")
    }
}

impl ComponentDefine for WasiRecordType {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, ";; record {}", self.symbol)?;
        w.newline()?;
        write!(w, "(type {} (record", self.symbol.wasi_id())?;
        w.indent();
        for field in self.fields.values() {
            w.newline()?;
            field.component_define(w)?;
        }
        w.dedent(2);
        Ok(())
        // (field "x" float32)
    }
}

impl ComponentDefine for WasiRecordField {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(field \"{}\" ", self.wasi_name)?;
        self.r#type.upper_type(w)?;
        write!(w, ")")
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
    fn upper_type_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }

    fn lower_type_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}
