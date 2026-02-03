use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_tcl::{TclBuilder, TclLanguage, TclRoot};
use oak_core::{Builder, SourceText};

pub struct RustyTclFrontend {
    language: TclLanguage,
}

impl RustyTclFrontend {
    pub fn new() -> Self {
        Self {
            language: TclLanguage::default(),
        }
    }
}

impl Default for RustyTclFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyTclFrontend {
    type Language = TclLanguage;

    fn parse(&self, source: &str) -> Result<TclRoot, NyarError> {
        let builder = TclBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<TclLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &TclRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现从 TclRoot 到 IKunTree 的转换
        Ok(IKunTree::Module("rusty-tcl-program".to_string(), Vec::new()))
    }
}
