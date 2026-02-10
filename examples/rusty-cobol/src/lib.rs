use nyar_types::{NyarContext, NyarError, NyarFrontend, Id, Vfs};
use oak_cobol::{CobolLanguage, parser::CobolParser};
use oak_core::{parser::Parser, SourceText};

pub struct RustyCobolFrontend {
    language: CobolLanguage,
}

impl RustyCobolFrontend {
    pub fn new() -> Self {
        Self {
            language: CobolLanguage::default(),
        }
    }
}

impl Default for RustyCobolFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyCobolFrontend {
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

    fn lower_unified<V: Vfs>(&self, _ast: &(), ctx: &mut NyarContext<V>) -> Id {
        // TODO: 实现从 COBOL AST 到 IKunTree 的转换
        ctx.builder.module("rusty-cobol-program", Vec::new(), nyar_types::Loc::default())
    }
}
