use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_scheme::{SchemeBuilder, SchemeLanguage};
use oak_core::{Builder, SourceText};

pub struct MiniSchemeFrontend {
    language: SchemeLanguage,
}

impl MiniSchemeFrontend {
    pub fn new() -> Self {
        Self {
            language: SchemeLanguage::default(),
        }
    }
}

impl Default for MiniSchemeFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for MiniSchemeFrontend {
    type Language = SchemeLanguage;

    fn parse(&self, source: &str) -> Result<(), NyarError> {
        let builder = SchemeBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<SchemeLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &()) -> Result<IKunTree, NyarError> {
        // TODO: 实现从 Scheme AST 到 IKunTree 的转换
        Ok(IKunTree::Module("mini-scheme-program".to_string(), Vec::new()))
    }
}
