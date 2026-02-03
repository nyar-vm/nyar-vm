//! Java 到 Nyar UIR 的转换器

use chomsky_types::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder, Id};
use nyar_types::NyarError;
use oak_java::ast::*;

/// Java 到 UIR 的转换器
pub struct JavaUirConverter<'a> {
    builder: IntentBuilder<'a, ConstraintAnalysis>,
    source_id: u32,
}

impl<'a> JavaUirConverter<'a> {
    /// 创建新的转换器
    pub fn new(egraph: &'a mut EGraph<IKun, ConstraintAnalysis>, source_id: u32) -> Self {
        Self {
            builder: IntentBuilder::new(egraph),
            source_id,
        }
    }

    /// 辅助方法：创建位置信息
    fn loc(&self) -> Loc {
        Loc::new(self.source_id, 0, 0)
    }

    /// 将 Java AST 转换为 UIR 树
    pub fn convert_to_tree(ast: &JavaRoot, source_id: u32) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let mut converter = Self::new(&mut egraph, source_id);
        let root_id = converter.convert_root(ast)?;

        if let Some(root_id) = root_id {
            let extractor = chomsky_extract::IKunExtractor::new(
                &egraph,
                chomsky_cost::DEFAULT_COST_MODEL.clone(),
            );
            Ok(extractor.extract(root_id))
        } else {
            Err(NyarError::Compile("No code generated".to_string()))
        }
    }

    /// 转换根节点
    pub fn convert_root(&mut self, root: &JavaRoot) -> Result<Option<Id>, NyarError> {
        let mut items = Vec::new();
        for item in &root.items {
            match item {
                Item::Class(class) => {
                    items.push(self.convert_class(class)?);
                }
                _ => {}
            }
        }
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.builder.seq(items, self.loc())))
        }
    }

    fn convert_class(&mut self, class: &ClassDeclaration) -> Result<Id, NyarError> {
        let mut members = Vec::new();
        for member in &class.members {
            match member {
                Member::Method(method) => {
                    members.push(self.convert_method(method)?);
                }
                Member::Field(field) => {
                    members.push(self.convert_field(field)?);
                }
            }
        }
        let name_id = self.builder.string(&class.name, self.loc());
        let members_id = self.builder.seq(members, self.loc());
        Ok(self.builder.extension("class", vec![name_id, members_id], self.loc()))
    }

    fn convert_field(&mut self, field: &FieldDeclaration) -> Result<Id, NyarError> {
        let name_id = self.builder.string(&field.name, self.loc());
        let type_id = self.builder.string(&field.r#type, self.loc());
        Ok(self.builder.extension("field", vec![name_id, type_id], self.loc()))
    }

    fn convert_method(&mut self, method: &MethodDeclaration) -> Result<Id, NyarError> {
        let body_id = self.convert_block(&method.body)?;
        let name_id = self.builder.string(&method.name, self.loc());
        let ret_id = self.builder.string(&method.return_type, self.loc());

        let mut param_ids = Vec::new();
        for param in &method.parameters {
            param_ids.push(self.convert_parameter(param)?);
        }
        let params_id = self.builder.seq(param_ids, self.loc());

        // 规范化 method 扩展：[name, params, return_type, body]
        Ok(self.builder.extension(
            "method",
            vec![name_id, params_id, ret_id, body_id],
            self.loc(),
        ))
    }

    fn convert_parameter(&mut self, param: &Parameter) -> Result<Id, NyarError> {
        let name_id = self.builder.string(&param.name, self.loc());
        let type_id = self.builder.string(&param.r#type, self.loc());
        Ok(self.builder.extension("parameter", vec![name_id, type_id], self.loc()))
    }

    fn convert_block(&mut self, stmts: &[Statement]) -> Result<Id, NyarError> {
        let mut ids = Vec::new();
        for stmt in stmts {
            ids.push(self.convert_stmt(stmt)?);
        }
        Ok(self.builder.seq(ids, self.loc()))
    }

    fn convert_stmt(&mut self, stmt: &Statement) -> Result<Id, NyarError> {
        match stmt {
            Statement::Expression(expr) => self.convert_expr(expr),
            Statement::Return(Some(expr)) => {
                let val = self.convert_expr(expr)?;
                Ok(self.builder.extension("return", vec![val], self.loc()))
            }
            Statement::Return(None) => Ok(self.builder.extension("return", vec![], self.loc())),
            Statement::Block(stmts) => self.convert_block(stmts),
        }
    }

    fn convert_expr(&mut self, expr: &Expression) -> Result<Id, NyarError> {
        match expr {
            Expression::Literal(Literal::Integer(v)) => Ok(self.builder.constant(*v, self.loc())),
            Expression::Literal(Literal::String(s)) => Ok(self.builder.string(s, self.loc())),
            Expression::Literal(Literal::Boolean(b)) => Ok(self.builder.bool(*b, self.loc())),
            Expression::Identifier(s) => {
                // 特殊处理 System.out
                if s == "System.out" {
                    let system = self.builder.symbol("System", self.loc());
                    let out = self.builder.symbol("out", self.loc());
                    return Ok(self.builder.extension("get_field", vec![system, out], self.loc()));
                }
                Ok(self.builder.symbol(s, self.loc()))
            }
            Expression::FieldAccess(fa) => {
                let target = self.convert_expr(&fa.target)?;
                let name = self.builder.symbol(&fa.name, self.loc());
                Ok(self.builder.extension("get_field", vec![target, name], self.loc()))
            }
            Expression::MethodCall(call) => {
                let mut arg_ids = Vec::new();
                for arg in &call.arguments {
                    arg_ids.push(self.convert_expr(arg)?);
                }

                // 特殊处理 System.out.println 和 System.out.print
                let mut is_std_io = false;
                if let Some(target) = &call.target {
                    match &**target {
                        Expression::Identifier(s) if s == "System.out" => is_std_io = true,
                        Expression::FieldAccess(fa) => {
                            if let Expression::Identifier(t) = &*fa.target {
                                if t == "System" && fa.name == "out" {
                                    is_std_io = true;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if is_std_io {
                    if call.name == "println" {
                        return Ok(self.builder.cross_lang_call(
                            "nyar",
                            "std::io::println",
                            arg_ids,
                            self.loc(),
                        ));
                    } else if call.name == "print" {
                        return Ok(self.builder.cross_lang_call(
                            "nyar",
                            "std::io::print",
                            arg_ids,
                            self.loc(),
                        ));
                    }
                }

                let name_id = self.builder.symbol(&call.name, self.loc());
                let args_id = self.builder.seq(arg_ids, self.loc());
                if let Some(target) = &call.target {
                    let target_id = self.convert_expr(target)?;
                    Ok(self.builder.extension(
                        "call",
                        vec![target_id, name_id, args_id],
                        self.loc(),
                    ))
                } else {
                    Ok(self.builder.extension("call", vec![name_id, args_id], self.loc()))
                }
            }
        }
    }
}
