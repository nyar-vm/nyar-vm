use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_prolog::{PrologBuilder, PrologLanguage, PrologRoot};
use oak_core::{Builder, SourceText};

pub struct MiniPrologFrontend {
    language: PrologLanguage,
}

impl MiniPrologFrontend {
    pub fn new() -> Self {
        Self {
            language: PrologLanguage::default(),
        }
    }
}

impl Default for MiniPrologFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for MiniPrologFrontend {
    type Language = PrologLanguage;

    fn parse(&self, source: &str) -> Result<PrologRoot, NyarError> {
        let builder = PrologBuilder::new(self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<PrologLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &PrologRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现从 PrologRoot 到 IKunTree 的转换
        Ok(IKunTree::Module("mini-prolog-program".to_string(), Vec::new()))
    }
}
