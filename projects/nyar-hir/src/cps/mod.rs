use crate::{
    ast::{LambdaFunction, LetBind, Symbol},
    ASTKind, ASTNode,
};

mod effects;

pub struct CpsTransformer {
    pub count: u64,
}

impl CpsTransformer {
    pub fn make_symbol(&mut self) -> Symbol {
        self.count += 1;
        Symbol::atom(format!("Φ_{}", self.count))
    }

    // function make_continuation(k) {
    // var cont = gensym('R');
    // return {
    // type: 'function',
    // vars: [cont],
    // body: k({
    // type: 'var',
    // value: cont
    // })
    // };
    // }

    pub fn make_continuation(&mut self, _k: LambdaFunction) -> LambdaFunction {
        let _cont = self.make_symbol();
        // LambdaFunction {
        //
        // }
        // k.call(cont)
        //
        todo!()
    }
}

impl CpsTransformer {
    pub fn cps(&mut self, ast: &ASTNode, k: LambdaFunction) -> ASTNode {
        match &ast.kind {
            ASTKind::Nothing => unreachable!(),
            ASTKind::Program(_) => {
                todo!()
            }
            ASTKind::Sequence(_nodes) => {
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
            ASTKind::Boolean(_) | ASTKind::Integer(_) | ASTKind::String(_) | ASTKind::Symbol(_) => k.call(ast),
            ASTKind::ListExpression(_) => {
                todo!()
            }
            ASTKind::TupleExpression(_) => {
                todo!()
            }

            ASTKind::DictExpression(_) => {
                todo!()
            }
            ASTKind::IfStatement(_) => {
                todo!()
            }
            ASTKind::InfixExpression(_) => unreachable!(),
            ASTKind::LoopStatement(_) => {
                unreachable!()
            }
            ASTKind::Suite(_) => {
                todo!()
            }
            ASTKind::Byte(_) => {
                todo!()
            }
        }
    }
    fn cps_let(&mut self, _ast: &LetBind, _k: LambdaFunction) -> ASTNode {
        todo!()
    }
    fn cps_lambda(&mut self, _ast: &LambdaFunction, _k: LambdaFunction) -> ASTNode {
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
    //                 type: 'function',
    //                 vars: [exp.vars[0].name],
    //                 body: {
    //                     type: 'let',
    //                     vars: exp.vars.slice(1),
    //                     body: exp.body
    //                 }
    //             }
    //         }, k);
    //     }
    pub fn cps_transform(&self, _ctx: &mut CpsTransformer) -> ASTKind {
        todo!()
    }
}
