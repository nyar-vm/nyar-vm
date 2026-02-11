use nyar_types::{NyarContext, NyarError, NyarFrontend, Id, Vfs};
use oak_wolfram::{WolframBuilder, WolframLanguage};
use oak_core::{Builder, SourceText};

pub struct RustyWolframFrontend {
    language: WolframLanguage,
}

impl RustyWolframFrontend {
    pub fn new() -> Self {
        Self {
            language: WolframLanguage::default(),
        }
    }
}

impl Default for RustyWolframFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyWolframFrontend {
    type Language = WolframLanguage;

    fn parse(&self, source: &str) -> Result<(), NyarError> {
        let builder = WolframBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<WolframLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, _ast: &(), ctx: &mut NyarContext<V>) -> Id {
        // TODO: 实现从 Wolfram AST 到 IKunTree 的转换
        ctx.builder.module("rusty-wolfram-program", Vec::new(), nyar_types::Loc::default())
    }
}
