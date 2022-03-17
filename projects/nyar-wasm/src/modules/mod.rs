use crate::{
    functions::{FunctionBuilder, FunctionItem},
    helpers::{WasmBuilder, WasmEmitter},
    TypeItem,
};
use indexmap::IndexMap;
use nyar_error::NyarError;
use nyar_hir::{GlobalBuilder, NamedValue};

use wasm_encoder::{CodeSection, ExportKind, ExportSection, Module, RefType, TableSection, TableType, TypeSection, ValType};

pub struct ModuleBuilder {
    globals: GlobalBuilder,
    types: IndexMap<String, TypeItem>,
    functions: FunctionBuilder,
}

impl ModuleBuilder {
    pub fn new() -> Self {
        let mut types = IndexMap::default();
        types.insert(
            "Func1".to_string(),
            TypeItem::Function { input: vec![ValType::I32, ValType::I32], output: vec![ValType::I32] },
        );
        types.insert("Func2".to_string(), TypeItem::Function { input: vec![ValType::I32], output: vec![ValType::I32] });

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

        Self { globals: Default::default(), types, functions: Default::default() }
    }

    fn build_types(&self) -> Result<TypeSection, NyarError> {
        let mut types = TypeSection::default();
        for (_, typing) in &self.types {
            typing.emit(&mut types, &())?
        }
        Ok(types)
    }
    pub fn insert_function(&mut self, function: FunctionItem) {
        self.functions.insert(function)
    }

    fn build_codes(&self) -> CodeSection {
        let mut codes = CodeSection::default();
        for func in self.functions.into_iter() {
            codes.function(&func.body);
        }
        codes
    }

    fn build_tables(&self) -> TableSection {
        let mut tables = TableSection::default();
        tables.table(TableType { element_type: RefType::FUNCREF, minimum: 0, maximum: None });
        tables
    }
    pub fn insert_global(&mut self, global: NamedValue) -> Option<NamedValue> {
        self.globals.insert(global)
    }

    fn build_exports(&self) -> ExportSection {
        let mut exports = ExportSection::default();
        for (index, func) in self.functions.into_iter().enumerate() {
            if func.export {
                exports.export(&func.name, ExportKind::Func, index as u32);
            }
        }
        exports
    }

    pub fn build(&self) -> Result<Vec<u8>, NyarError> {
        // https://webassembly.github.io/spec/core/binary/modules.html
        let mut module = Module::new();
        // The build order cannot be reversed!!!
        module.section(&self.build_types()?);
        module.section(&self.functions.build());
        module.section(&self.build_tables());
        module.section(&self.globals.build(&self.functions)?);
        module.section(&self.build_exports());
        module.section(&self.build_codes());
        Ok(module.finish())
    }
}
