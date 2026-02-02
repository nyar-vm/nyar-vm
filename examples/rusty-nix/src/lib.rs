use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_nix::{NixBuilder, NixLanguage};
use oak_core::{Builder, SourceText};

pub struct MiniNixFrontend {
    language: NixLanguage,
}

impl MiniNixFrontend {
    pub fn new() -> Self {
        Self {
            language: NixLanguage::default(),
        }
    }
}

impl Default for MiniNixFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for MiniNixFrontend {
    type Language = NixLanguage;

    fn parse(&self, source: &str) -> Result<(), NyarError> {
        let builder = NixBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<NixLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &()) -> Result<IKunTree, NyarError> {
        // TODO: 实现从 Nix AST 到 IKunTree 的转换
        Ok(IKunTree::Module("mini-nix-program".to_string(), Vec::new()))
    }
}
