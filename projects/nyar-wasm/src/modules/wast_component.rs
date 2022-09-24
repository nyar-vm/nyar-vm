use super::*;
use wast::component::{CoreInstance, CoreInstanceExport, CoreInstanceKind, CoreItemRef, ItemRef};

impl<'a, 'i> IntoWasm<'a, Option<NameAnnotation<'i>>> for WasmBuilder
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Option<NameAnnotation<'i>> {
        if self.name.is_empty() { None } else { Some(NameAnnotation { name: self.name.as_str() }) }
    }
}

impl<'a, 'i> IntoWasm<'a, Producers<'i>> for WasmBuilder
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Producers<'i> {
        Producers {
            fields: vec![
                ("language", vec![("valkyrie", "2024"), ("player", "berserker")]),
                ("processed-by", vec![("nyar-wasm", env!("CARGO_PKG_VERSION"))]),
            ],
        }
    }
}

impl<'a, 'i> IntoWasm<'a, CoreModule<'i>> for WasmBuilder
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreModule<'i> {
        CoreModule {
            span: Span::from_offset(0),
            id: WasmName::id(self.name.as_str()),
            name: None,
            exports: Default::default(),
            kind: self.as_wast(),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, CoreModuleKind<'i>> for WasmBuilder
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreModuleKind<'i> {
        let module = self.as_module();
        match module.kind {
            ModuleKind::Text(v) => CoreModuleKind::Inline { fields: v },
            ModuleKind::Binary(_) => unreachable!(),
        }
    }
}

impl WasmBuilder {
    pub fn build_component<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, NyarError> {
        let mut module = self.as_component()?;
        write_wasm_bytes(path.as_ref(), module.encode())
    }

    pub fn as_component(&self) -> Result<Component, NyarError> {
        let mut coms = vec![];
        // for ts in self.structures.values() {
        // coms.push(ComponentField::Type(ts.as_wast()));
        // coms.push(ComponentField::CoreType(ts.as_wast()))
        // }
        coms.push(ComponentField::CoreModule(self.as_wast()));
        // (core instance $library (instantiate $Library))
        coms.push(ComponentField::CoreInstance(CoreInstance {
            span: Span::from_offset(0),
            id: None,
            name: Some(NameAnnotation { name: "instance_name" }),
            kind: CoreInstanceKind::Instantiate {
                module: ItemRef {
                    kind: Default::default(),
                    idx: Index::Id(WasmName::id("add_random_pi").unwrap()),
                    export_names: vec![],
                },
                args: vec![],
            },
        }));

        for fs in self.functions.values() {
            if !self.entry.is_empty() {
                // coms.push(ComponentField::Start(fs.as_wast()));
            }
            // coms.push(ComponentField::CoreFunc(fs.as_wast()));
        }
        for fs in self.functions.values() {
            if !self.entry.is_empty() {
                // coms.push(ComponentField::Start(fs.as_wast()));
            }
            coms.extend(fs.make_component());
        }
        coms.push(ComponentField::Producers(self.as_wast()));
        Ok(Component { span: Span::from_offset(0), id: None, name: None, kind: ComponentKind::Text(coms) })
    }
}
