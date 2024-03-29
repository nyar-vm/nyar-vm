use super::*;

use wast::component::{
    ComponentExportType, ComponentExternName, ComponentImport, ComponentTypeUse, CoreInstance, CoreInstanceKind,
    CoreInstantiationArg, CoreInstantiationArgKind, InstanceType, InstanceTypeDecl, ItemRef, ItemSig, ItemSigKind,
};

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

impl<'a, 'i> IntoWasm<'a, CoreInstance<'i>> for WasmBuilder
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreInstance<'i> {
        let mut args = vec![];
        for (name, functions) in self.import_groups() {
            let mut exports = vec![];
            for function in functions {
                exports.push(function.as_wast())
            }

            args.push(CoreInstantiationArg {
                name: name.long_name(),
                kind: CoreInstantiationArgKind::BundleOfExports(Span::from_offset(0), exports),
            })
        }

        CoreInstance {
            span: Span::from_offset(0),
            id: WasmName::id(self.get_instance_name()),
            name: None,
            kind: CoreInstanceKind::Instantiate {
                module: ItemRef {
                    kind: Default::default(),
                    idx: WasmName::index(self.get_module_name()),
                    export_names: vec![],
                },
                args,
            },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, CoreModule<'i>> for WasmBuilder
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreModule<'i> {
        let module = self.as_module();
        let kind = match module.kind {
            ModuleKind::Text(v) => CoreModuleKind::Inline { fields: v },
            ModuleKind::Binary(_) => unreachable!(),
        };
        CoreModule {
            span: Span::from_offset(0),
            id: WasmName::id(self.get_module_name()),
            name: None,
            exports: Default::default(),
            kind,
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
        // Import external functions to valkyrie module
        for (name, functions) in self.import_groups() {
            let mut decls = vec![];
            for f in functions {
                // decls.push(InstanceTypeDecl::Type(f.output.as_wast()));
                let export = InstanceTypeDecl::Export(ComponentExportType {
                    span: Span::from_offset(0),
                    name: ComponentExternName(f.external_name.as_ref()),
                    item: f.as_wast(),
                });
                decls.push(export);
            }

            coms.push(ComponentField::Import(ComponentImport {
                span: Span::from_offset(0),
                name: ComponentExternName(name.long_name()),
                item: ItemSig {
                    span: Span::from_offset(0),
                    id: WasmName::id(name.long_name()),
                    name: None,
                    kind: ItemSigKind::Instance(ComponentTypeUse::Inline(InstanceType { decls })),
                },
            }))
        }

        for function in self.externals.values() {
            coms.push(ComponentField::CoreFunc(function.as_wast()));
        }
        // Export valkyrie module externally
        coms.push(ComponentField::CoreModule(self.as_wast()));
        coms.push(ComponentField::CoreInstance(self.as_wast()));
        for fs in self.functions.values() {
            if !self.entry.is_empty() {
                // coms.push(ComponentField::Start(fs.as_wast()));
            }
            coms.extend(fs.make_component(self.get_instance_name()));
        }
        coms.push(ComponentField::Producers(self.as_wast()));
        Ok(Component { span: Span::from_offset(0), id: None, name: None, kind: ComponentKind::Text(coms) })
    }
}
