use crate::{
    helpers::{WasmBuilder, WasmEmitter},
    TypeItem,
};
use indexmap::IndexMap;
use nyar_error::NyarError;
use nyar_hir::{FunctionExternalItem, FunctionItem, FunctionRegister, GlobalBuilder, Identifier, NamedValue};

use crate::{
    functions::WasmFunctionBody,
    types::{TypeBuilder, WasmFunction},
    values::WasmVariable,
};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, ElementSection, Elements, EntityType, ExportKind, ExportSection, FunctionSection,
    GlobalSection, ImportSection, MemorySection, MemoryType, Module, RefType, StartSection, TableSection, TableType,
    TypeSection,
};

#[derive(Default)]
pub struct ModuleBuilder {
    memory_pages: u64,
    globals: GlobalBuilder,
    types: TypeBuilder,
    data: DataBuilder,
    functions: FunctionRegister,
}
#[derive(Default)]
pub struct DataBuilder {
    data: IndexMap<String, DataItem>,
}

pub struct DataItem {
    name: Identifier,
    data: Vec<u8>,
}

impl DataBuilder {
    pub fn insert(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item.name.to_string(), item)
    }
}

impl DataItem {
    pub fn utf8(name: Identifier, data: String) -> Self {
        Self { name, data: data.into_bytes() }
    }
}

impl ModuleBuilder {
    pub fn new(memory: u64) -> Self {
        // let mut fields = IndexMap::default();
        // fields.insert("x".to_string(), FieldType { element_type: StorageType::I8, mutable: true });
        // fields.insert("y".to_string(), FieldType { element_type: StorageType::I8, mutable: true });
        // types.insert("Point".to_string(), TypeBuilder::Structure {
        //     fields,
        // });

        // types.insert("utf8".to_string(), TypeBuilder::Array {
        //     raw: StorageType::I8,
        // });
        //
        // types.insert("ST".to_string(), TypeBuilder::SubTyping {
        //     sub: SubType {
        //         is_final: false,
        //         supertype_idx: None,
        //         composite_type: CompositeType::Struct(StructType { fields: Box::new([FieldType { element_type: StorageType::I8, mutable: false }]) }),
        //     },
        // });

        Self { memory_pages: memory, ..Default::default() }
    }

    pub fn insert_type(&mut self, ty: TypeItem) -> Option<TypeItem> {
        self.types.insert(ty)
    }
    pub fn insert_function(&mut self, f: FunctionItem) {
        self.functions.add_native(f)
    }
    pub fn insert_external(&mut self, f: FunctionExternalItem) {
        self.functions.add_external(f)
    }

    pub fn insert_data(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item)
    }

    pub fn insert_global(&mut self, global: NamedValue) -> Option<NamedValue> {
        self.globals.insert(global)
    }
}

impl ModuleBuilder {
    fn build_start(&self, m: &mut Module) {
        for (index, _, value) in self.functions.get_natives() {
            if value.entry {
                m.section(&StartSection { function_index: index as u32 });
            }
        }
    }
    fn build_exports(&self, m: &mut Module) {
        let mut exports = ExportSection::default();
        for (index, _, func) in self.functions.get_natives() {
            if func.export {
                exports.export(&func.namepath.to_string(), ExportKind::Func, index as u32);
            }
        }

        for (index, _, func) in self.globals.into_iter() {
            if func.export {
                exports.export(&func.namepath.to_string(), ExportKind::Global, index as u32);
            }
        }
        m.section(&exports);
    }
    fn build_import(&self, m: &mut Module) {
        let mut imports = ImportSection::default();
        for (index, _, func) in self.functions.get_externals() {
            imports.import(&func.module.to_string(), &func.field.to_string(), EntityType::Function(index as u32));
        }
        m.section(&imports);
    }
    fn build_types(&self, m: &mut Module) {
        let mut types = TypeSection::default();
        for (_, _, func) in self.functions.get_externals() {
            func.emit_function(&mut types)
        }
        for (_, _, typing) in self.functions.get_natives() {
            typing.emit_function(&mut types)
        }
        for (_, _, typing) in &self.types {
            typing.emit(&mut types, &()).unwrap()
        }
        m.section(&types);
    }
    fn build_functions(&self, m: &mut Module) {
        let mut imports = FunctionSection::default();
        for (i, _, _) in self.functions.get_natives() {
            imports.function(i as u32);
        }
        m.section(&imports);
    }
    fn build_codes(&self, m: &mut Module) -> Result<(), NyarError> {
        let mut codes = CodeSection::default();
        for (_, _, func) in self.functions.get_natives() {
            codes.function(&func.body.emit_function_body()?);
        }
        m.section(&codes);
        Ok(())
    }

    fn build_memory(&self, m: &mut Module) {
        let mut memory = MemorySection::new();
        memory.memory(MemoryType { minimum: 1, maximum: Some(self.memory_pages), memory64: false, shared: true });
        m.section(&memory);
    }

    fn build_data(&self, m: &mut Module) {
        let mut data = DataSection::default();
        let mut offset = 0;
        for (_, item) in self.data.data.iter() {
            data.active(0, &ConstExpr::i32_const(offset as i32), item.data.iter().copied());
            offset += item.data.len();
        }
        m.section(&data);
    }

    fn build_globals(&self, m: &mut Module) {
        let mut global = GlobalSection::default();
        for (_, _, value) in self.globals.into_iter() {
            value.emit_global(&mut global, &self.functions).unwrap()
        }
        m.section(&global);
    }
    fn build_tables(&self, m: &mut Module) {
        let mut tables = TableSection::default();
        tables.table(TableType { element_type: RefType::FUNCREF, minimum: 128, maximum: None });
        m.section(&tables);
    }

    fn build_elements(&self, m: &mut Module) {
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
        let mut module = Module::new();
        self.build_types(&mut module);
        self.build_import(&mut module);
        self.build_functions(&mut module);
        self.build_tables(&mut module);
        self.build_memory(&mut module);
        self.build_globals(&mut module);
        self.build_exports(&mut module);
        self.build_start(&mut module);
        self.build_elements(&mut module);
        self.build_codes(&mut module)?;
        self.build_data(&mut module);
        Ok(module.finish())
    }
}
