use crate::{modules::GlobalValue, TypeBuilder};
use indexmap::IndexMap;
use wasm_encoder::{
    ArrayType, CodeSection, CompositeType, ConstExpr, ExportKind, ExportSection, FieldType, Function, FunctionSection,
    GlobalSection, GlobalType, Instruction, Module, RefType, StorageType, StructType, SubType, TableSection, TableType,
    TypeSection, ValType,
};

pub struct ModuleBuilder {
    globals: IndexMap<String, GlobalValue>,
    types: IndexMap<String, TypeBuilder>,
    functions: IndexMap<String, FunctionBuilder>,
}

pub struct FunctionBuilder {
    name: String,
    export: bool,
    typing: String,
    body: Function,
}

impl ModuleBuilder {
    pub fn new() -> Self {
        let mut types = IndexMap::default();
        types.insert(
            "Func1".to_string(),
            TypeBuilder::Function { input: vec![ValType::I32, ValType::I32], output: vec![ValType::I32] },
        );
        types.insert("Func2".to_string(), TypeBuilder::Function { input: vec![ValType::I32], output: vec![ValType::I32] });

        // let mut fields = IndexMap::default();
        // fields.insert("x".to_string(), FieldType { element_type: StorageType::I8, mutable: true });
        // fields.insert("y".to_string(), FieldType { element_type: StorageType::I8, mutable: true });
        // types.insert("Point".to_string(), TypeBuilder::Structure {
        //     fields,
        // });

        // types.insert("utf8".to_string(), TypeBuilder::Array {
        //     raw: StorageType::I8,
        // });

        // types.insert("ST".to_string(), TypeBuilder::SubTyping {
        //     sub: SubType {
        //         is_final: false,
        //         supertype_idx: None,
        //         composite_type: CompositeType::Struct(StructType { fields: Box::new([FieldType { element_type: StorageType::I8, mutable: false }]) }),
        //     },
        // });

        let mut functions = IndexMap::default();

        let locals = vec![];
        let mut body1 = Function::new(locals);
        body1.instruction(&Instruction::LocalGet(0));
        body1.instruction(&Instruction::LocalGet(1));
        body1.instruction(&Instruction::I32Add);
        body1.instruction(&Instruction::End);

        functions.insert(
            "add_ab".to_string(),
            FunctionBuilder { name: "add_ab".to_string(), export: false, typing: "Func".to_string(), body: body1.clone() },
        );

        let locals = vec![];
        let mut body2 = Function::new(locals);
        body2.instruction(&Instruction::LocalGet(0));
        body2.instruction(&Instruction::GlobalGet(0));
        body2.instruction(&Instruction::I32Add);
        body2.instruction(&Instruction::End);
        functions.insert(
            "add_ba".to_string(),
            FunctionBuilder { name: "add_ba".to_string(), export: true, typing: "Func".to_string(), body: body2.clone() },
        );
        Self { globals: Default::default(), types, functions }
    }

    pub fn insert_global(&mut self, global: GlobalValue) {
        self.globals.insert(global.name.clone(), global);
    }

    fn build_types(&self) -> TypeSection {
        let mut types = TypeSection::default();
        for (_, typing) in &self.types {
            typing.build(&mut types)
        }
        types
    }

    fn build_functions(&self) -> FunctionSection {
        let mut functions = FunctionSection::default();
        for i in 0..self.functions.len() {
            functions.function(i as u32);
        }
        functions
    }

    fn build_codes(&self) -> CodeSection {
        let mut codes = CodeSection::default();
        for func in self.functions.values() {
            codes.function(&func.body);
        }
        codes
    }

    fn build_tables(&self) -> TableSection {
        let mut tables = TableSection::default();
        tables.table(TableType { element_type: RefType::FUNCREF, minimum: 0, maximum: None });
        tables
    }

    fn build_global(&self) -> GlobalSection {
        let mut global = GlobalSection::default();

        global.global(GlobalType { val_type: ValType::I32, mutable: true }, &ConstExpr::i32_const(42));
        global
    }

    fn build_exports(&self) -> ExportSection {
        let mut exports = ExportSection::default();
        for (index, func) in self.functions.values().enumerate() {
            if func.export {
                exports.export(&func.name, ExportKind::Func, index as u32);
            }
        }
        exports
    }

    pub fn build(&self) -> Vec<u8> {
        // https://webassembly.github.io/spec/core/binary/modules.html
        let mut module = Module::new();
        // The build order cannot be reversed!!!
        module.section(&self.build_types());
        module.section(&self.build_functions());
        module.section(&self.build_tables());
        module.section(&self.build_global());
        module.section(&self.build_exports());
        module.section(&self.build_codes());
        module.finish()
    }
}
