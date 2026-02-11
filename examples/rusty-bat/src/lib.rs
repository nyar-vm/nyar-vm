use chomsky_uir::{IKun, Id};
use nyar_types::{NyarContext, NyarError, NyarFrontend};
use oak_bat::ast::{BatRoot, Element};
use oak_bat::{BatBuilder, BatLanguage};
use oak_core::{Builder, SourceText};
use oak_vfs::Vfs;

pub struct RustyBatFrontend {
    language: BatLanguage,
}

impl RustyBatFrontend {
    pub fn new() -> Self {
        Self {
            language: BatLanguage::default(),
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

impl Default for RustyBatFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyBatFrontend {
    type Language = BatLanguage;

    fn parse(&self, source: &str) -> Result<BatRoot, NyarError> {
        let builder = BatBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<BatLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &BatRoot, ctx: &mut NyarContext<V>) -> Id {
        let items: Vec<Id> = ast
            .elements
            .iter()
            .map(|e| self.lower_element(e, ctx))
            .collect();
        ctx.egraph.add(IKun::Seq(items))
    }
}
