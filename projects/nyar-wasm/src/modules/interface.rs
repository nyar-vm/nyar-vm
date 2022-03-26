use super::*;

impl ModuleBuilder {
    fn build_start(&self, m: &mut wasm_encoder::Module) {
        for (index, _, value) in self.functions.get_natives() {
            if value.entry {
                m.section(&StartSection { function_index: index as u32 });
            }
        }
    }
    fn build_exports(&self, m: &mut wasm_encoder::Module) {
        let mut exports = ExportSection::default();
        for (index, _, func) in self.functions.get_natives() {
            if func.export {
                exports.export(&func.symbol.to_string(), ExportKind::Func, index as u32);
            }
        }

        for (index, _, func) in self.globals.into_iter() {
            if func.export {
                exports.export(&func.symbol.to_string(), ExportKind::Global, index as u32);
            }
        }
        m.section(&exports);
    }
    fn build_codes(&self, m: &mut wasm_encoder::Module) -> Result<(), NyarError> {
        let mut codes = CodeSection::default();
        for (_, _, func) in self.functions.get_natives() {
            codes.function(&func.body.emit_function_body()?);
        }
        m.section(&codes);
        Ok(())
    }
    fn build_memory(&self, m: &mut wasm_encoder::Module) {
        let mut memory = MemorySection::new();
        memory.memory(MemoryType { minimum: 1, maximum: Some(self.memory_pages), memory64: false, shared: true });
        m.section(&memory);
    }
    fn build_data(&self, m: &mut wasm_encoder::Module) {
        let mut data = DataSection::default();
        let mut offset = 0;
        for (_, item) in self.data.data.iter() {
            data.active(0, &ConstExpr::i32_const(offset as i32), item.data.iter().copied());
            offset += item.data.len();
        }
        m.section(&data);
    }
    fn build_tables(&self, m: &mut wasm_encoder::Module) {
        let mut tables = TableSection::default();
        tables.table(TableType { element_type: RefType::FUNCREF, minimum: 128, maximum: None });
        m.section(&tables);
    }

    fn build_elements(&self, m: &mut wasm_encoder::Module) {
        let mut elements = ElementSection::default();
        let offset = ConstExpr::i32_const(42);
        let functions = Elements::Functions(&[
            // Function indices...
        ]);
        elements.active(Some(0), &offset, functions);
        m.section(&elements);
    }

    pub fn build(&self) -> Result<Vec<u8>, NyarError> {
        // The build order cannot be reversed!!!
        let mut module = wasm_encoder::Module::new();
        // self.build_types(&mut module);
        // self.build_import(&mut module);
        // self.build_functions(&mut module);
        // self.build_tables(&mut module);
        // self.build_memory(&mut module);
        // self.build_globals(&mut module);
        // self.build_exports(&mut module);
        // self.build_start(&mut module);
        // self.build_elements(&mut module);
        // self.build_codes(&mut module)?;
        // self.build_data(&mut module);
        Ok(module.finish())
    }
}
