//! Rusty TypeScript 语言前端
//!
//! 这个库提供了 Rusty TypeScript 语言的解析和 Nyar 翻译功能。
//! 遵循 Project Chomsky Whitebook 规范。

#![feature(new_range_api)]

pub mod codegen;
pub mod errors;
pub mod project;
pub mod type_system;

use chomsky_cost;
use chomsky_emit::GaiaEmitter;
use chomsky_extract::{Backend, IKunExtractor};
use chomsky_types::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, Id, IntentBuilder, IKunTree};
use nyar_aot::NyarAot;
use nyar_types::{NyarError, NyarFrontend};
use nyar_vm::bytecode::format::NyarcModule;
use oak_core::{ParseSession, SourceText};
use oak_typescript::{ast, TypeScriptBuilder, TypeScriptLanguage, TypeScriptRoot};
use std::ops::Range;

/// Rusty TypeScript 前端
pub struct RustyTypescriptFrontend {
    language: TypeScriptLanguage,
    source_id: u32,
}

impl Default for RustyTypescriptFrontend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
wit_bindgen::generate!({
    world: "compiler",
    path: "wit",
});

#[cfg(target_arch = "wasm32")]
struct Compiler;

#[cfg(target_arch = "wasm32")]
impl Guest for Compiler {
    fn compile(source: String) -> Result<Vec<u8>, String> {
        let frontend = RustyTypescriptFrontend::new();
        let artifacts = frontend.compile_to_wasm(&source)?;
        
        // Return main.wasm or the first artifact
        if let Some(wasm) = artifacts.get("main.wasm") {
            Ok(wasm.clone())
        } else if let Some((_, bytes)) = artifacts.iter().next() {
            Ok(bytes.clone())
        } else {
            Err("No output files generated".to_string())
        }
    }
}

#[cfg(target_arch = "wasm32")]
export!(Compiler);

impl RustyTypescriptFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let mut language = TypeScriptLanguage::standard();
        language.decorators = true;
        language.jsx = true;
        Self {
            language,
            source_id: 1, // 默认 source_id
        }
    }

    /// 设置当前处理的源码 ID
    pub fn set_source_id(&mut self, id: u32) {
        self.source_id = id;
    }

    /// 编译源码为 WASM (AOT)
    pub fn compile_to_wasm(&self, source: &str) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
        let tree = self.lower_to_tree(source)?;

        let mut aot = NyarAot::<ConstraintAnalysis>::new();
        let emitter = GaiaEmitter::new("wasm32-wasi").standalone();

        // 使用 AOT 编译器进行优化和生成
        // 由于 GaiaEmitter 实现了 Backend 接口，可以直接调用 generate
        let artifact = emitter.generate(&tree)
            .map_err(|e| format!("AOT error: {:?}", e))?;

        match artifact {
            chomsky_extract::BackendArtifact::Binary(bytes) => {
                let mut files = std::collections::HashMap::new();
                files.insert("main.wasm".to_string(), bytes);
                Ok(files)
            }
            chomsky_extract::BackendArtifact::Collection(files) => Ok(files),
            _ => Err("Expected binary artifact from AOT compiler".to_string()),
        }
    }

    fn lower_to_tree(&self, source: &str) -> Result<IKunTree, String> {
        let ast = self
            .parse(source)
            .map_err(|e| format!("Parse error: {:?}", e))?;
        let vfs = oak_vfs::MemoryVfs::new();
        self.lower(&ast, &vfs)
            .map_err(|e| format!("Lowering error: {:?}", e))
    }

    /// 编译源码为 Nyar 模块
    pub fn compile_to_nyar(&self, source: &str) -> Result<NyarcModule, String> {
        let tree = self.lower_to_tree(source)?;

        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = tree.to_egraph(&mut egraph);

        let mut translator = codegen::NyarTranslator::new();
        translator
            .generate(&egraph, root_id)
            .map_err(|e| format!("Codegen error: {:?}", e))
    }
}

impl NyarFrontend for RustyTypescriptFrontend {
    type Language = TypeScriptLanguage;

    fn parse(&self, source: &str) -> Result<TypeScriptRoot, NyarError> {
        let builder = TypeScriptBuilder::new(&self.language);
        let mut session = ParseSession::<TypeScriptLanguage>::default();
        let source_text = SourceText::new(source);
        let output = oak_core::Builder::build(&builder, &source_text, &[], &mut session);

        output
            .result
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower<V: oak_vfs::Vfs>(&self, ast: &TypeScriptRoot, _vfs: &V) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let mut builder = IntentBuilder::new(&mut egraph);
        let mut converter = UirConverter::new(&mut builder, self.source_id);

        let root_id = converter.convert_root(ast.clone());
        let extractor = IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
        let tree = extractor.extract(root_id);
        Ok(tree)
    }
}

struct UirConverter<'a> {
    builder: &'a mut IntentBuilder<'a, ConstraintAnalysis>,
    source_id: u32,
}

impl<'a> UirConverter<'a> {
    fn new(builder: &'a mut IntentBuilder<'a, ConstraintAnalysis>, source_id: u32) -> Self {
        Self { builder, source_id }
    }

    fn to_loc(&self, range: Range<usize>) -> Loc {
        Loc::new(self.source_id, range.start as u32, range.end as u32)
    }

    fn convert_root(&mut self, root: TypeScriptRoot) -> Id {
        let mut items = Vec::new();
        for stmt in root.statements {
            items.push(self.convert_statement(stmt));
        }
        self.builder.module("main", items)
    }

    fn convert_statement(&mut self, stmt: ast::Statement) -> Id {
        let span = stmt.span();
        match stmt {
            ast::Statement::VariableDeclaration(var) => {
                let loc = self.to_loc(var.span.into());
                let value = if let Some(expr) = var.value {
                    self.convert_expression(expr)
                } else {
                    self.builder.constant(0, loc.clone())
                };
                let ty = var.ty.map(|t| self.convert_type_annotation(t, loc.clone()));
                let mut args = vec![self.builder.symbol(&var.name, loc.clone()), value];
                if let Some(ty_id) = ty {
                    args.push(ty_id);
                }
                args.push(self.builder.bool(var.is_declare, loc.clone()));
                self.builder.extension("assign", args, loc)
            }
            ast::Statement::FunctionDeclaration(func) => {
                let loc = self.to_loc(func.span.clone().into());
                let mut body_ids = Vec::new();
                for s in func.body {
                    body_ids.push(self.convert_statement(s));
                }
                let mut lambda_args = vec![
                    self.builder.string(&func.name, loc.clone()),
                    self.builder.seq(
                        func.type_params
                            .into_iter()
                            .map(|tp| self.convert_type_parameter(tp, loc.clone()))
                            .collect(),
                        loc.clone(),
                    ),
                    self.builder.seq(
                        func.params
                            .into_iter()
                            .map(|p| {
                                let p_loc = loc.clone();
                                let mut p_args = vec![self.builder.symbol(&p.name, p_loc.clone())];
                                if let Some(ty) = p.ty {
                                    p_args.push(self.convert_type_annotation(ty, p_loc.clone()));
                                }
                                p_args.push(self.builder.bool(p.optional, p_loc.clone()));
                                self.builder.extension("param", p_args, p_loc)
                            })
                            .collect(),
                        loc.clone(),
                    ),
                    self.builder.block(body_ids, loc.clone()),
                ];

                if let Some(ret) = func.return_type {
                    lambda_args.push(self.convert_type_annotation(ret, loc.clone()));
                }

                let lambda = self.builder.extension("function", lambda_args, loc.clone());
                let mut assign_args = vec![self.builder.symbol(&func.name, loc.clone()), lambda];
                assign_args.push(self.builder.bool(func.is_declare, loc.clone()));
                self.builder.extension("assign", assign_args, loc)
            }
            ast::Statement::ExpressionStatement(expr) => self.convert_expression(expr),
            ast::Statement::ImportDeclaration(import) => {
                let loc = self.to_loc(import.span.into());
                let mut args = vec![self.builder.string(&import.module_specifier, loc.clone())];
                for s in import.imports {
                    args.push(self.builder.symbol(&s, loc.clone()));
                }
                self.builder.extension("import", args, loc)
            }
            ast::Statement::ExportDeclaration(export) => {
                let loc = self.to_loc(export.span.into());
                let inner = self.convert_statement(*export.declaration);
                self.builder.extension("export", vec![inner], loc)
            }
            ast::Statement::ReturnStatement(value) => {
                let loc = self.to_loc(span.into());
                let val = if let Some(expr) = value {
                    self.convert_expression(expr)
                } else {
                    self.builder.constant(0, loc.clone())
                };
                self.builder.return_(val, loc)
            }
            ast::Statement::ClassDeclaration(class) => {
                let loc = self.to_loc(class.span.into());
                let mut args = vec![self.builder.symbol(&class.name, loc.clone())];

                // Type parameters
                args.push(self.builder.seq(
                    class
                        .type_params
                        .into_iter()
                        .map(|tp| self.convert_type_parameter(tp, loc.clone()))
                        .collect(),
                    loc.clone(),
                ));

                if let Some(ext) = class.extends {
                    args.push(self.convert_type_annotation(ext, loc.clone()));
                } else {
                    args.push(self.builder.constant(0, loc.clone())); // No base class
                }

                // Add abstract and implements
                args.push(self.builder.bool(class.is_abstract, loc.clone()));
                let implements_ids: Vec<_> = class
                    .implements
                    .into_iter()
                    .map(|imp| self.convert_type_annotation(imp, loc.clone()))
                    .collect();
                args.push(self.builder.seq(implements_ids, loc.clone()));

                for member in class.body {
                    args.push(self.convert_class_member(member, loc.clone()));
                }
                args.push(self.builder.bool(class.is_declare, loc.clone()));
                self.builder.extension("gc.struct", args, loc)
            }
            ast::Statement::Namespace(ns) => {
                let loc = self.to_loc(ns.span.into());
                let mut items = Vec::new();
                for s in ns.body {
                    items.push(self.convert_statement(s));
                }
                let mut args = vec![self.builder.string(&ns.name, loc.clone())];
                args.push(self.builder.seq(items, loc.clone()));
                args.push(self.builder.bool(ns.is_declare, loc.clone()));
                self.builder.extension("module", args, loc)
            }
            ast::Statement::Interface(interface) => {
                let loc = self.to_loc(interface.span.into());
                let mut args = vec![self.builder.symbol(&interface.name, loc.clone())];

                // Type parameters
                args.push(self.builder.seq(
                    interface
                        .type_params
                        .into_iter()
                        .map(|tp| self.convert_type_parameter(tp, loc.clone()))
                        .collect(),
                    loc.clone(),
                ));

                let extends_ids: Vec<_> = interface
                    .extends
                    .into_iter()
                    .map(|ext| self.convert_type_annotation(ext, loc.clone()))
                    .collect();
                args.push(self.builder.seq(extends_ids, loc.clone()));

                for member in interface.body {
                    args.push(self.convert_class_member(member, loc.clone()));
                }
                args.push(self.builder.bool(interface.is_declare, loc.clone()));
                self.builder.extension("interface", args, loc)
            }
            ast::Statement::TypeAlias(alias) => {
                let loc = self.to_loc(alias.span.into());
                let name = self.builder.symbol(&alias.name, loc.clone());

                // Type parameters
                let type_params = self.builder.seq(
                    alias
                        .type_params
                        .into_iter()
                        .map(|tp| self.convert_type_parameter(tp, loc.clone()))
                        .collect(),
                    loc.clone(),
                );

                let ty = self.convert_type_annotation(alias.ty, loc.clone());
                self.builder.extension("type_alias", vec![name, type_params, ty, self.builder.bool(alias.is_declare, loc.clone())], loc)
            }
            ast::Statement::Enum(enum_decl) => {
                let loc = self.to_loc(enum_decl.span.into());
                let mut args = vec![self.builder.symbol(&enum_decl.name, loc.clone())];
                for member in enum_decl.members {
                    let mut m_args = vec![self.builder.symbol(&member.name, loc.clone())];
                    if let Some(init) = member.initializer {
                        m_args.push(self.convert_expression(init));
                    }
                    args.push(self.builder.extension("enum_member", m_args, loc.clone()));
                }
                args.push(self.builder.bool(enum_decl.is_declare, loc.clone()));
                self.builder.extension("enum", args, loc)
            }
            ast::Statement::IfStatement(stmt) => {
                let loc = self.to_loc(stmt.span.into());
                let cond = self.convert_expression(stmt.test);
                let then_branch = self.convert_statement(*stmt.consequent);
                let else_branch = if let Some(alt) = stmt.alternate {
                    self.convert_statement(*alt)
                } else {
                    self.builder.constant(0, loc.clone())
                };
                self.builder.branch(cond, then_branch, else_branch, loc)
            }
            ast::Statement::WhileStatement(stmt) => {
                let loc = self.to_loc(stmt.span.into());
                let cond = self.convert_expression(stmt.test);
                let body = self.convert_statement(*stmt.body);
                self.builder.while_loop(cond, body, loc)
            }
            ast::Statement::BlockStatement(stmt) => {
                let loc = self.to_loc(stmt.span.into());
                let mut stmts = Vec::new();
                for s in stmt.statements {
                    stmts.push(self.convert_statement(s));
                }
                self.builder.block(stmts, loc)
            }
            ast::Statement::BreakStatement => {
                let loc = self.to_loc(span.into());
                self.builder.extension("break", vec![], loc)
            }
            ast::Statement::ContinueStatement => {
                let loc = self.to_loc(span.into());
                self.builder.extension("continue", vec![], loc)
            }
            ast::Statement::ThrowStatement(expr) => {
                let loc = self.to_loc(span.into());
                let val = self.convert_expression(expr);
                self.builder.extension("throw", vec![val], loc)
            }
            ast::Statement::DoWhileStatement(stmt) => {
                let loc = self.to_loc(stmt.span.into());
                let cond = self.convert_expression(stmt.test);
                let body = self.convert_statement(*stmt.body);
                self.builder.extension("do_while", vec![cond, body], loc)
            }
            ast::Statement::ForStatement(stmt) => {
                let loc = self.to_loc(stmt.span.into());
                let init = if let Some(init) = stmt.initializer {
                    self.convert_statement(*init)
                } else {
                    self.builder.constant(0, loc.clone())
                };
                let test = if let Some(test) = stmt.test {
                    self.convert_expression(test)
                } else {
                    self.builder.bool(true, loc.clone())
                };
                let update = if let Some(update) = stmt.incrementor {
                    self.convert_expression(update)
                } else {
                    self.builder.constant(0, loc.clone())
                };
                let body = self.convert_statement(*stmt.body);
                self.builder
                    .extension("for_loop", vec![init, test, update, body], loc)
            }
            ast::Statement::ForInStatement(stmt) => {
                let loc = self.to_loc(stmt.span.into());
                let left = self.convert_statement(*stmt.left);
                let right = self.convert_expression(stmt.right);
                let body = self.convert_statement(*stmt.body);
                self.builder.extension("for_in", vec![left, right, body], loc)
            }
            ast::Statement::ForOfStatement(stmt) => {
                let loc = self.to_loc(stmt.span.into());
                let left = self.convert_statement(*stmt.left);
                let right = self.convert_expression(stmt.right);
                let body = self.convert_statement(*stmt.body);
                self.builder.extension("for_of", vec![left, right, body], loc)
            }
            ast::Statement::SwitchStatement(stmt) => {
                let loc = self.to_loc(stmt.span.into());
                let discriminant = self.convert_expression(stmt.discriminant);
                let mut cases = Vec::new();
                for case in stmt.cases {
                    let case_loc = self.to_loc(case.span.into());
                    let test = if let Some(test) = case.test {
                        self.convert_expression(test)
                    } else {
                        self.builder.symbol("default", case_loc.clone())
                    };
                    let mut body = Vec::new();
                    for s in case.consequent {
                        body.push(self.convert_statement(s));
                    }
                    let body_id = self.builder.block(body, case_loc.clone());
                    cases.push(self.builder.extension("case", vec![test, body_id], case_loc));
                }
                let mut args = vec![discriminant];
                args.extend(cases);
                self.builder.extension("switch", args, loc)
            }
            ast::Statement::TryStatement(stmt) => {
                let loc = self.to_loc(stmt.span.clone().into());
                let block = self.convert_statement(ast::Statement::BlockStatement(ast::BlockStatement {
                    statements: stmt.block,
                    span: stmt.span.clone(),
                }));
                let mut args = vec![block];

                if let Some(handler) = stmt.handler {
                    let handler_loc = self.to_loc(handler.span.into());
                    let param_name = handler.param.unwrap_or_else(|| "error".to_string());
                    let param = self.builder.symbol(&param_name, handler_loc.clone());
                    let body = self.convert_statement(ast::Statement::BlockStatement(ast::BlockStatement {
                        statements: handler.body,
                        span: handler.span.clone(),
                    }));
                    args.push(self.builder.extension("catch", vec![param, body], handler_loc));
                }

                if let Some(finalizer) = stmt.finalizer {
                    let finalizer_loc = self.to_loc(stmt.span.clone().into()); // Use stmt span for finalizer if not available
                    let body = self.convert_statement(ast::Statement::BlockStatement(ast::BlockStatement {
                        statements: finalizer,
                        span: stmt.span.clone(),
                    }));
                    args.push(self.builder.extension("finally", vec![body], finalizer_loc));
                }

                self.builder.extension("try", args, loc)
            }
        }
    }

    fn convert_expression(&mut self, expr: ast::Expression) -> Id {
        match expr {
            ast::Expression::Identifier(name) => self.builder.symbol(&name, Loc::default()),
            ast::Expression::NumericLiteral(val) => self.builder.constant(val as i64, Loc::default()),
            ast::Expression::StringLiteral(val) => self.builder.string(&val, Loc::default()),
            ast::Expression::BooleanLiteral(val) => self.builder.bool(val, Loc::default()),
            ast::Expression::NullLiteral => self.builder.constant(0, Loc::default()),
            ast::Expression::BigIntLiteral(val) => self.builder.extension("bigint", vec![self.builder.string(&val, Loc::default())], Loc::default()),
            ast::Expression::RegexLiteral(val) => self.builder.extension("regex", vec![self.builder.string(&val, Loc::default())], Loc::default()),
            ast::Expression::TemplateString(val) => self.builder.extension("template", vec![self.builder.string(&val, Loc::default())], Loc::default()),
            ast::Expression::BinaryExpression {
                left,
                operator,
                right,
            } => {
                let l = self.convert_expression(*left);
                let r = self.convert_expression(*right);
                self.builder.binary_op(&operator, l, r, Loc::default())
            }
            ast::Expression::CallExpression { func, args } => {
                let span = func.span();
                let loc = self.to_loc(span.into());

                // Detect console.log and map to std::io::println
                if let ast::Expression::MemberExpression {
                    object,
                    property,
                    computed,
                    ..
                } = func.as_ref()
                {
                    if !*computed {
                        if let (
                            ast::Expression::Identifier(obj_name),
                            ast::Expression::Identifier(prop_name),
                        ) = (object.as_ref(), property.as_ref())
                        {
                            if obj_name == "console" {
                                let mut arg_ids = Vec::new();
                                for arg in &args {
                                    arg_ids.push(self.convert_expression(arg.clone()));
                                }
                                if prop_name == "log" || prop_name == "println" {
                                    return self.builder.cross_lang_call(
                                        "nyar",
                                        "std::io",
                                        "println",
                                        arg_ids,
                                        loc,
                                    );
                                } else if prop_name == "print" {
                                    return self.builder.cross_lang_call(
                                        "nyar",
                                        "std::io",
                                        "print",
                                        arg_ids,
                                        loc,
                                    );
                                }
                            }
                        }
                    }
                }

                let f = self.convert_expression(*func);
                let mut arg_ids = Vec::new();
                for arg in args {
                    arg_ids.push(self.convert_expression(arg));
                }
                self.builder.call(f, arg_ids, Loc::default())
            }
            ast::Expression::UnaryExpression { operator, argument } => {
                let arg = self.convert_expression(*argument);
                self.builder.extension(&operator, vec![arg], Loc::default())
            }
            ast::Expression::MemberExpression {
                object,
                property,
                computed,
                optional,
            } => {
                let obj = self.convert_expression(*object);
                let prop = self.convert_expression(*property);
                let mut args = vec![obj, prop];
                args.push(self.builder.bool(optional, Loc::default()));
                if computed {
                    self.builder.extension("index", args, Loc::default())
                } else {
                    self.builder.extension("gc.get_field", args, Loc::default())
                }
            }
            ast::Expression::ConditionalExpression {
                test,
                consequent,
                alternate,
            } => {
                let t = self.convert_expression(*test);
                let c = self.convert_expression(*consequent);
                let a = self.convert_expression(*alternate);
                self.builder.branch(t, c, a, Loc::default())
            }
            ast::Expression::NewExpression { func, args } => {
                let f = self.convert_expression(*func);
                let mut arg_ids = vec![f];
                for arg in args {
                    arg_ids.push(self.convert_expression(arg));
                }
                self.builder.extension("gc.new", arg_ids, Loc::default())
            }
            ast::Expression::AssignmentExpression {
                left,
                operator,
                right,
            } => {
                let r = self.convert_expression(*right);
                if operator == "=" {
                    match *left {
                        ast::Expression::MemberExpression {
                            object, property, ..
                        } => {
                            let obj = self.convert_expression(*object);
                            let prop = self.convert_expression(*property);
                            self.builder.extension(
                                "gc.set_field",
                                vec![obj, prop, r],
                                Loc::default(),
                            )
                        }
                        _ => {
                            let l = self.convert_expression(*left);
                            self.builder.assign_to_id(l, r, Loc::default())
                        }
                    }
                } else {
                    let l = self.convert_expression(*left.clone());
                    // Compound assignment like +=
                    let op = operator.trim_end_matches('=');
                    let value = self.builder.binary_op(op, l, r, Loc::default());
                    let target = self.convert_expression(*left);
                    self.builder.assign_to_id(target, value, Loc::default())
                }
            }
            ast::Expression::AsExpression {
                expression,
                type_annotation,
            } => {
                let expr = self.convert_expression(*expression);
                let ty = self.convert_type_annotation(type_annotation, Loc::default());
                self.builder.extension("as", vec![expr, ty], Loc::default())
            }
            ast::Expression::ImportExpression {
                module_specifier,
                span,
            } => {
                let loc = self.to_loc(span.into());
                let spec = self.convert_expression(*module_specifier);
                self.builder.extension("import", vec![spec], loc)
            }
            ast::Expression::ArrowFunction {
                type_params,
                params,
                return_type,
                body,
                async_,
            } => {
                let body_id = self.convert_statement(*body);
                let mut args = vec![
                    self.builder.seq(
                        type_params
                            .into_iter()
                            .map(|tp| self.convert_type_parameter(tp, Loc::default()))
                            .collect(),
                        Loc::default(),
                    ),
                    self.builder.seq(
                        params
                            .into_iter()
                            .map(|p| {
                                let mut p_args = vec![self.builder.symbol(&p.name, Loc::default())];
                                if let Some(ty) = p.ty {
                                    p_args.push(self.convert_type_annotation(ty, Loc::default()));
                                }
                                p_args.push(self.builder.bool(p.optional, Loc::default()));
                                self.builder.extension("param", p_args, Loc::default())
                            })
                            .collect(),
                        Loc::default(),
                    ),
                    body_id,
                ];
                if let Some(ret) = return_type {
                    args.push(self.convert_type_annotation(ret, Loc::default()));
                }
                if async_ {
                    self.builder.extension("async_lambda", args, Loc::default())
                } else {
                    self.builder.extension("lambda", args, Loc::default())
                }
            }
            ast::Expression::ObjectLiteral { properties } => {
                let mut args = Vec::new();
                for prop in properties {
                    match prop {
                        ast::ObjectProperty::Property { name, value } => {
                            let key = self.builder.symbol(&name, Loc::default());
                            let value = self.convert_expression(value);
                            args.push(self.builder.extension("prop", vec![key, value], Loc::default()));
                        }
                        ast::ObjectProperty::ShorthandProperty(name) => {
                            let key = self.builder.symbol(&name, Loc::default());
                            let value = self.builder.symbol(&name, Loc::default());
                            args.push(self.builder.extension("prop", vec![key, value], Loc::default()));
                        }
                        ast::ObjectProperty::SpreadProperty(expr) => {
                            let inner = self.convert_expression(expr);
                            args.push(self.builder.extension("spread_prop", vec![inner], Loc::default()));
                        }
                    }
                }
                self.builder.extension("object", args, Loc::default())
            }
            ast::Expression::ArrayLiteral { elements } => {
                let mut args = Vec::new();
                for elem in elements {
                    args.push(self.convert_expression(elem));
                }
                self.builder.extension("array", args, Loc::default())
            }
            ast::Expression::SpreadElement(expr) => {
                let inner = self.convert_expression(*expr);
                self.builder.extension("spread", vec![inner], Loc::default())
            }
            ast::Expression::AwaitExpression(expr) => {
                let inner = self.convert_expression(*expr);
                self.builder.extension("await", vec![inner], Loc::default())
            }
            ast::Expression::YieldExpression(expr) => {
                let mut args = Vec::new();
                if let Some(e) = expr {
                    args.push(self.convert_expression(*e));
                }
                self.builder.extension("yield", args, Loc::default())
            }
            _ => self.builder.constant(0, Loc::default()),
        }
    }

    fn convert_type_annotation(&mut self, ty: ast::TypeAnnotation, loc: Loc) -> Id {
        match ty {
            ast::TypeAnnotation::Identifier(name) => self.builder.symbol(&name, loc),
            ast::TypeAnnotation::Predefined(name) => self.builder.symbol(&name, loc),
            ast::TypeAnnotation::Literal(lit) => {
                let lit_id = match lit {
                    ast::LiteralType::String(s) => self.builder.string(&s, loc.clone()),
                    ast::LiteralType::Number(n) => self.builder.constant(n as i64, loc.clone()),
                    ast::LiteralType::Boolean(b) => self.builder.bool(b, loc.clone()),
                    ast::LiteralType::BigInt(s) => {
                        self.builder.extension("bigint", vec![self.builder.string(&s, loc.clone())], loc.clone())
                    }
                };
                self.builder.extension("literal_type", vec![lit_id], loc)
            }
            ast::TypeAnnotation::Array(inner) => {
                let inner_id = self.convert_type_annotation(*inner, loc.clone());
                self.builder.extension("array_type", vec![inner_id], loc)
            }
            ast::TypeAnnotation::Tuple(elements) => {
                let elem_ids = elements.into_iter().map(|e| self.convert_type_annotation(e, loc.clone())).collect();
                let seq = self.builder.seq(elem_ids, loc.clone());
                self.builder.extension("tuple_type", vec![seq], loc)
            }
            ast::TypeAnnotation::Union(types) => {
                let type_ids = types.into_iter().map(|t| self.convert_type_annotation(t, loc.clone())).collect();
                let seq = self.builder.seq(type_ids, loc.clone());
                self.builder.extension("union_type", vec![seq], loc)
            }
            ast::TypeAnnotation::Intersection(types) => {
                let type_ids = types.into_iter().map(|t| self.convert_type_annotation(t, loc.clone())).collect();
                let seq = self.builder.seq(type_ids, loc.clone());
                self.builder.extension("intersection_type", vec![seq], loc)
            }
            ast::TypeAnnotation::Reference { name, args } => {
                let name_id = self.builder.symbol(&name, loc.clone());
                let arg_ids = args.into_iter().map(|a| self.convert_type_annotation(a, loc.clone())).collect();
                let seq = self.builder.seq(arg_ids, loc.clone());
                self.builder.extension("type_ref", vec![name_id, seq], loc)
            }
            ast::TypeAnnotation::Function { params: _, args, return_type } => {
                let param_ids = args
                    .into_iter()
                    .map(|p| {
                        let mut p_args = vec![self.builder.symbol(&p.name, loc.clone())];
                        if let Some(ty) = p.ty {
                            p_args.push(self.convert_type_annotation(ty, loc.clone()));
                        }
                        p_args.push(self.builder.bool(p.optional, loc.clone()));
                        self.builder.extension("param", p_args, loc.clone())
                    })
                    .collect();
                let params_seq = self.builder.seq(param_ids, loc.clone());
                let ret_id = self.convert_type_annotation(*return_type, loc.clone());
                self.builder.extension("function_type", vec![params_seq, ret_id], loc)
            }
            ast::TypeAnnotation::Object(members) => {
                let member_ids = members.into_iter().map(|m| self.convert_class_member(m, loc.clone())).collect();
                let seq = self.builder.seq(member_ids, loc.clone());
                self.builder.extension("object_type", vec![seq], loc)
            }
            ast::TypeAnnotation::Query(name) => {
                let name_id = self.builder.symbol(&name, loc.clone());
                self.builder.extension("typeof", vec![name_id], loc)
            }
            ast::TypeAnnotation::KeyOf(inner) => {
                let inner_id = self.convert_type_annotation(*inner, loc.clone());
                self.builder.extension("keyof", vec![inner_id], loc)
            }
        }
    }

    fn convert_type_parameter(&mut self, tp: ast::TypeParameter, loc: Loc) -> Id {
        let mut args = vec![self.builder.symbol(&tp.name, loc.clone())];
        if let Some(constraint) = tp.constraint {
            args.push(self.convert_type_annotation(constraint, loc.clone()));
        } else {
            args.push(self.builder.constant(0, loc.clone()));
        }
        if let Some(default) = tp.default {
            args.push(self.convert_type_annotation(default, loc.clone()));
        } else {
            args.push(self.builder.constant(0, loc.clone()));
        }
        self.builder.extension("type_param", args, loc)
    }

    fn convert_class_member(&mut self, member: ast::ClassMember, loc: Loc) -> Id {
        match member {
            ast::ClassMember::Property {
                decorators: _,
                name,
                ty,
                initializer,
                visibility,
                is_static,
                is_readonly,
                is_abstract,
                is_optional,
                span: _,
            } => {
                let mut args = vec![self.builder.symbol(&name, loc.clone())];
                if let Some(ty) = ty {
                    args.push(self.convert_type_annotation(ty, loc.clone()));
                } else {
                    args.push(self.builder.constant(0, loc.clone()));
                }
                if let Some(val) = initializer {
                    args.push(self.convert_expression(val));
                } else {
                    args.push(self.builder.constant(0, loc.clone()));
                }
                args.push(self.builder.bool(is_static, loc.clone()));
                args.push(self.builder.bool(is_readonly, loc.clone()));
                args.push(self.builder.bool(is_abstract, loc.clone()));
                args.push(self.builder.string(&format!("{:?}", visibility), loc.clone()));
                args.push(self.builder.bool(is_optional, loc.clone()));
                self.builder.extension("property", args, loc)
            }
            ast::ClassMember::Method {
                decorators: _,
                name,
                type_params,
                params,
                return_type,
                body,
                visibility,
                is_static,
                is_abstract,
                is_getter,
                is_setter,
                is_optional,
                span: _,
            } => {
                let mut args = vec![self.builder.symbol(&name, loc.clone())];
                args.push(self.builder.seq(
                    type_params
                        .into_iter()
                        .map(|tp| self.convert_type_parameter(tp, loc.clone()))
                        .collect(),
                    loc.clone(),
                ));
                args.push(self.builder.seq(
                    params
                        .into_iter()
                        .map(|p| {
                            let mut p_args = vec![self.builder.symbol(&p.name, loc.clone())];
                            if let Some(ty) = p.ty {
                                p_args.push(self.convert_type_annotation(ty, loc.clone()));
                            }
                            p_args.push(self.builder.bool(p.optional, loc.clone()));
                            self.builder.extension("param", p_args, loc.clone())
                        })
                        .collect(),
                    loc.clone(),
                ));
                if let Some(ret) = return_type {
                    args.push(self.convert_type_annotation(ret, loc.clone()));
                } else {
                    args.push(self.builder.constant(0, loc.clone()));
                }
                let body_ids = body.into_iter().map(|s| self.convert_statement(s)).collect();
                args.push(self.builder.block(body_ids, loc.clone()));
                args.push(self.builder.bool(is_static, loc.clone()));
                args.push(self.builder.bool(is_abstract, loc.clone()));
                args.push(self.builder.bool(is_getter, loc.clone()));
                args.push(self.builder.bool(is_setter, loc.clone()));
                args.push(self.builder.string(&format!("{:?}", visibility), loc.clone()));
                args.push(self.builder.bool(is_optional, loc.clone()));
                self.builder.extension("method", args, loc)
            }
        }
    }
}
