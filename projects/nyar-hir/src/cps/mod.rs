use crate::{
    ast::{ASTAtom, LambdaFunction, LetBind},
    ASTKind, ASTNode,
};

pub struct CpsTransformer {
    pub count: u64,
}

impl CpsTransformer {
    pub fn new_symbol() -> Self {
        Self { count: 0 }
    }
}

impl ASTNode {
    pub fn cps_transform(&self, ctx: &mut CpsTransformer) -> ASTNode {
        ASTNode { kind: self.kind.cps_transform(ctx), meta: self.meta.clone() }
    }
}

impl ASTKind {
    pub fn cps_transform(&self, ctx: &mut CpsTransformer) -> ASTKind {
        match self {
            ASTKind::Nothing => ASTKind::Nothing,
            ASTKind::ASTAtom(atom) => atom.cps_transform(ctx),
            ASTKind::LetBind(bind) => bind.cps_transform(ctx),
            ASTKind::Sequence(nodes) => ASTKind::Sequence(nodes.iter().map(|f| f.cps_transform(ctx)).collect()),
            ASTKind::LambdaFunction(lambda) => lambda.cps_transform(ctx),
        }
    }
}

impl ASTAtom {
    pub fn cps_transform(&self, ctx: &mut CpsTransformer) -> ASTKind {
        todo!()
    }
}

impl LetBind {
    //     function cps_let(exp, k) {
    //         if (exp.vars.length === 0) {
    //             return cps(exp.body, k);
    //         }
    //         return cps({
    //             type: 'call',
    //             args: [exp.vars[0].def || FALSE],
    //             func: {
    //                 type: 'lambda',
    //                 vars: [exp.vars[0].name],
    //                 body: {
    //                     type: 'let',
    //                     vars: exp.vars.slice(1),
    //                     body: exp.body
    //                 }
    //             }
    //         }, k);
    //     }
    pub fn cps_transform(&self, ctx: &mut CpsTransformer) -> ASTKind {
        todo!()
    }
}

impl LambdaFunction {
    pub fn cps_transform(&self, ctx: &mut CpsTransformer) -> ASTKind {
        todo!()
    }
}
