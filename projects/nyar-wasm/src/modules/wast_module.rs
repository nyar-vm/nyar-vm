use super::*;

impl WasmBuilder {
    pub fn build_module<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, NyarError> {
        let mut module = self.as_module();
        write_wasm_bytes(path.as_ref(), module.encode())
    }
    pub fn as_module(&self) -> Module {
        let mut terms = Vec::with_capacity(1024);
        // imports
        for k in self.externals.values() {
            // match &k.output {
            //     WasmType::Structure(v) => {
            //         terms.push(ModuleField::Type(v.as_wast()));
            //     }
            //     _ => continue,
            // }

            terms.push(ModuleField::Import(k.as_wast()))
        }
        // types
        for ty in self.structures.values() {
            terms.push(ModuleField::Type(ty.as_wast()))
        }
        for ty in self.arrays.values() {
            terms.push(ModuleField::Type(ty.as_wast()))
        }
        // functions
        for ft in self.functions.values() {
            terms.push(ModuleField::Func(ft.as_wast()))
        }
        // variables
        for k in self.globals.values() {
            terms.push(ModuleField::Global(k.as_wast()))
        }
        // memory section
        self.build_memory(&mut terms);
        // data section, 一页 = 64KB, 从第二页开始存用户数据
        let mut offset = 65536;
        for k in self.data.values() {
            terms.push(ModuleField::Data(k.as_wast(&mut offset)))
        }
        // start section
        if !self.entry.is_empty() {
            terms.push(ModuleField::Start(WasmName::index(&self.entry)))
        }
        Module {
            span: Span::from_offset(0),
            id: None,
            name: Some(NameAnnotation { name: self.get_module_name() }),
            kind: ModuleKind::Text(terms),
        }
    }
    fn build_memory<'a, 'b>(&'a self, m: &mut Vec<ModuleField<'b>>)
    where
        'a: 'b,
    {
        let memory = Memory {
            span: Span::from_offset(0),
            id: WasmName::id("memory"),
            name: None,
            exports: InlineExport { names: vec!["memory"] },
            kind: MemoryKind::Normal(MemoryType::B32 { limits: Limits { min: 2, max: None }, shared: false }),
        };
        m.push(ModuleField::Memory(memory));
    }
}
