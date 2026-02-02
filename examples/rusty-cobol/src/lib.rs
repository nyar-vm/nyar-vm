use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_cobol::{CobolLanguage, parser::CobolParser};
use oak_core::{parser::Parser, SourceText};

pub struct MiniCobolFrontend {
    language: CobolLanguage,
}

impl MiniCobolFrontend {
    pub fn new() -> Self {
        Self {
            language: CobolLanguage::default(),
        }
    }
}

impl Default for MiniCobolFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for MiniCobolFrontend {
    type Language = CobolLanguage;

    fn parse(&self, source: &str) -> Result<(), NyarError> {
        let parser = CobolParser::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<CobolLanguage>::default();

        let output = parser.parse(&source_text, &[], &mut session);
        output
            .result
            .map(|_| ())
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &()) -> Result<IKunTree, NyarError> {
        // TODO: 实现从 COBOL AST 到 IKunTree 的转换
        Ok(IKunTree::Module("mini-cobol-program".to_string(), Vec::new()))
    }
}
