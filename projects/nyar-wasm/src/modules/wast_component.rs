use super::*;

impl<'a, 'i> IntoWasm<'a, ComponentValType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentValType<'i> {
        ComponentValType::Inline(self.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentDefinedType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentDefinedType<'i> {
        match self {
            Self::Structure(v) => ComponentDefinedType::Record(v.as_wast()),
            _ => ComponentDefinedType::Primitive(self.as_wast()),
        }
    }
}

impl<'a> IntoWasm<'a, PrimitiveValType> for WasmType {
    fn as_wast(&'a self) -> PrimitiveValType {
        match self {
            Self::Bool => PrimitiveValType::Bool,
            Self::U8 => PrimitiveValType::U8,
            Self::U16 => PrimitiveValType::U16,
            Self::U32 => PrimitiveValType::U32,
            Self::U64 => PrimitiveValType::U64,
            Self::I8 => PrimitiveValType::S8,
            Self::I16 => PrimitiveValType::S16,
            Self::I32 => PrimitiveValType::S32,
            Self::I64 => PrimitiveValType::S64,
            Self::F32 => PrimitiveValType::Float32,
            Self::F64 => PrimitiveValType::Float64,
            Self::Unicode => PrimitiveValType::Char,
            Self::UTF8Text => PrimitiveValType::String,
            _ => unreachable!("`{:?}` is not primitive type", self),
        }
    }
}

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
        for ts in self.types.values() {
            coms.push(ComponentField::Type(ts.as_wast()))
        }
        for fs in self.functions.values() {
            if !self.entry.is_empty() {
                coms.push(ComponentField::Start(fs.as_wast()));
                coms.push(ComponentField::Func(fs.as_wast()));
            }

            // coms.push(ComponentField::CoreFunc(fs.as_wast()));
        }
        coms.push(ComponentField::CoreModule(self.as_wast()));

        coms.push(ComponentField::Producers(self.as_wast()));
        Ok(Component { span: Span::from_offset(0), id: None, name: None, kind: ComponentKind::Text(coms) })
    }
}