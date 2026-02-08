use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_nim::{NimBuilder, NimLanguage, NimRoot};
use oak_core::{Builder, SourceText};

pub struct RustyNimFrontend {
    language: NimLanguage,
}

impl RustyNimFrontend {
    pub fn new() -> Self {
        Self {
            language: NimLanguage::default(),
        }
    }
}

impl Default for RustyNimFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyNimFrontend {
    type Language = NimLanguage;

    fn parse(&self, source: &str) -> Result<NimRoot, NyarError> {
        let builder = NimBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<NimLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &NimRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现从 NimRoot 到 IKunTree 的转换
        Ok(IKunTree::Module("rusty-nim-program".to_string(), Vec::new()))
    }
}
