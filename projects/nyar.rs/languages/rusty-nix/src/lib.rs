#![deny(missing_docs)]
use chomsky_uir::Id;
use nyar_types::{NyarContext, TypesError, NyarFrontend};
use data_text_core::{Builder, SourceText};
use data_text_nix::{NixBuilder, NixLanguage};
use data_text_core::vfs::Vfs;

pub struct RustyNixFrontend {
    language: NixLanguage,
}

impl RustyNixFrontend {
    pub fn new() -> Self {
        Self { language: NixLanguage::default() }
    }
}

impl Default for RustyNixFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyNixFrontend {
    type Language = NixLanguage;

    fn parse(&self, source: &str) -> Result<(), TypesError> {
        let builder = NixBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = data_text_core::parser::ParseSession::<NixLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| TypesError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, _ast: &(), ctx: &mut NyarContext<V>) -> Id {
        // TODO: 实现从 Nix AST 到 IKunTree 的转换
        ctx.builder.module("rusty-nix-program", Vec::new(), nyar_types::Loc::default())
    }
}
