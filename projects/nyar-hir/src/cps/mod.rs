use crate::{
    ast::{ASTMeta, InfixExpression, LambdaFunction, LetBind, Symbol},
    ASTKind, ASTNode,
};

mod effects;

pub struct CpsTransformer {
    pub count: u64,
}

impl CpsTransformer {
    pub fn make_symbol(&mut self) -> Symbol {
        self.count += 1;
        Symbol::simple(format!("Φ_{}", self.count))
    }

    // function make_continuation(k) {
    // var cont = gensym('R');
    // return {
    // type: 'lambda',
    // vars: [cont],
    // body: k({
    // type: 'var',
    // value: cont
    // })
    // };
    // }

    pub fn make_continuation(&mut self, k: LambdaFunction) -> LambdaFunction {
        let cont = self.make_symbol();
        // LambdaFunction {
        //
        // }
        // k.call(cont)
        //
        todo!()
    }
}

impl CpsTransformer {
    pub fn transform(&mut self, ast: &ASTNode, k: LambdaFunction) -> ASTNode {
        match &ast.kind {
            ASTKind::Nothing => unreachable!(),
            ASTKind::Program(_) => {
                todo!()
            }
            ASTKind::Sequence(nodes) => {
                todo!()
            }
            ASTKind::LetBind(bind) => self.cps_let(bind, k),
            ASTKind::LambdaFunction(lambda) => self.cps_lambda(lambda, k),
            ASTKind::XMLTemplate(_) => {
                todo!()
            }
            ASTKind::StringTemplate(_) => {
                todo!()
            }
            ASTKind::Boolean(_) | ASTKind::Number(_) | ASTKind::String(_) | ASTKind::Symbol(_) => k.call(ast),
            ASTKind::ListExpression(_) => {
                todo!()
            }
            ASTKind::TupleExpression(_) => {
                todo!()
            }

            ASTKind::DictExpression(_) => {
                todo!()
            }
            ASTKind::BinaryExpression(infix) => {
                let k = LambdaFunction {};

                if infix {}

                let r = LambdaFunction {};
                let rest = self.transform(&infix.rhs, r);

                self.transform(&infix.lhs, k)
            }
        }
    }
    fn cps_let(&mut self, ast: &LetBind, k: LambdaFunction) -> ASTNode {
        todo!()
    }
    fn cps_lambda(&mut self, ast: &LambdaFunction, k: LambdaFunction) -> ASTNode {
        todo!()
    }
}

impl InfixExpression {
    fn k_lhs(&self) {
        if self.o.is_infix_and() {}
    }
    fn k_rhs(&self) {}
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
