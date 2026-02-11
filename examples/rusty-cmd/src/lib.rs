use chomsky_uir::{IKun, Id};
use nyar_types::{NyarContext, NyarError, NyarFrontend};
use oak_cmd::ast::{CmdRoot, Element};
use oak_cmd::{CmdBuilder, CmdLanguage};
use oak_core::{Builder, SourceText};
use oak_vfs::Vfs;

pub struct RustyCmdFrontend {
    language: CmdLanguage,
}

impl RustyCmdFrontend {
    pub fn new() -> Self {
        Self {
            language: CmdLanguage::default(),
        }
    }

    fn lower_element<V: Vfs>(&self, element: &Element, ctx: &mut NyarContext<V>) -> Id {
        match element {
            Element::Command(cmd) => {
                let callee = ctx.egraph.add(IKun::Symbol(cmd.clone()));
                ctx.egraph.add(IKun::Apply(callee, vec![]))
            }
            Element::Variable(v) => ctx.egraph.add(IKun::Symbol(v.clone())),
            Element::String(s) => ctx.egraph.add(IKun::StringConstant(s.clone())),
            Element::Text(t) => ctx.egraph.add(IKun::StringConstant(t.clone())),
            _ => ctx.egraph.add(IKun::Seq(vec![])),
        }
    }
}

impl Default for RustyCmdFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyCmdFrontend {
    type Language = CmdLanguage;

    fn parse(&self, source: &str) -> Result<CmdRoot, NyarError> {
        let builder = CmdBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<CmdLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &CmdRoot, ctx: &mut NyarContext<V>) -> Id {
        let items: Vec<Id> = ast
            .elements
            .iter()
            .map(|e| self.lower_element(e, ctx))
            .collect();
        ctx.egraph.add(IKun::Seq(items))
    }
}
