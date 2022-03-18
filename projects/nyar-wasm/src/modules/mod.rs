use crate::{
    functions::{FunctionBuilder, FunctionItem},
    helpers::{WasmBuilder, WasmEmitter},
    TypeItem,
};
use indexmap::IndexMap;
use nyar_error::NyarError;
use nyar_hir::{ArrayType, GlobalBuilder, Identifier, NamedValue, StructureType};

use crate::types::{RecursiveType, TypeBuilder};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, DataSegment, ExportKind, ExportSection, MemorySection, MemoryType, Module, RefType,
    SubType, TableSection, TableType, TypeSection, ValType,
};

#[derive(Default)]
pub struct ModuleBuilder {
    globals: GlobalBuilder,
    types: TypeBuilder,
    data: DataBuilder,
    functions: FunctionBuilder,
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
    pub fn new() -> Self {
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

        todo!()
    }

    pub fn insert_type(&mut self, ty: TypeItem) -> Option<TypeItem> {
        self.types.insert(ty)
    }

    fn build_types(&self) -> Result<TypeSection, NyarError> {
        let mut types = TypeSection::default();
        for (_, _, typing) in self.functions.into_iter() {
            typing.typing.emit(&mut types, &())?
        }

        for (_, _, typing) in &self.types {
            typing.emit(&mut types, &())?
        }
        Ok(types)
    }
    pub fn insert_function(&mut self, function: FunctionItem) {
        self.functions.insert(function)
    }

    fn build_codes(&self) -> CodeSection {
        let mut codes = CodeSection::default();
        for (_, _, func) in self.functions.into_iter() {
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

    pub fn insert_data(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item)
    }

    fn build_data(&self) -> Result<DataSection, NyarError> {
        let mut data = DataSection::default();
        let mut index = 0;
        let mut offset = 0;
        for (_, item) in self.data.data.iter() {
            data.active(index, &ConstExpr::i32_const(offset as i32), item.data.iter().copied());
            index += 1;
            offset += item.data.len();
        }
        Ok(data)
    }
    fn build_exports(&self) -> ExportSection {
        let mut exports = ExportSection::default();
        for (index, _, func) in self.functions.into_iter() {
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
        let mut memory = MemorySection::new();
        memory.memory(MemoryType { minimum: 1, maximum: Some(10), memory64: false, shared: true });
        module.section(&memory);

        module.section(&self.globals.build(&self.functions)?);

        module.section(&self.build_exports());
        module.section(&self.build_codes());

        module.section(&self.build_data()?);

        Ok(module.finish())
    }
}
