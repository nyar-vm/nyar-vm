//! Mini TypeScript 语言前端
//!
//! 这个库提供了 Mini TypeScript 语言的解析和 Nyar 翻译功能。
//! 遵循 Project Chomsky Whitebook 规范。

#![feature(new_range_api)]

pub mod codegen;
pub mod errors;
pub mod project;
pub mod type_system;

use chomsky_cost;
use chomsky_emit::GaiaEmitter;
use chomsky_extract::{Backend, IKunExtractor};
use chomsky_source::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, Id, IntentBuilder, IKunTree};
use nyar_aot::NyarAot;
use nyar_types::{NyarError, NyarFrontend};
use nyar_vm::bytecode::format::NyarModule;
use oak_core::{ParseSession, SourceText};
use oak_typescript::{ast, TypeScriptBuilder, TypeScriptLanguage, TypeScriptRoot};
use std::ops::Range;

/// Mini TypeScript 前端
pub struct MiniTypescriptFrontend {
    language: TypeScriptLanguage,
    source_id: u32,
}

impl Default for MiniTypescriptFrontend {
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
        let frontend = MiniTypescriptFrontend::new();
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

impl MiniTypescriptFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: TypeScriptLanguage::standard(),
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
        let emitter = GaiaEmitter::new("wasm32-wasi");

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
        self.lower(&ast)
            .map_err(|e| format!("Lowering error: {:?}", e))
    }

    /// 编译源码为 Nyar 模块
    pub fn compile_to_nyar(&self, source: &str) -> Result<NyarModule, String> {
        let tree = self.lower_to_tree(source)?;

        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = tree.to_egraph(&mut egraph);

        let mut translator = codegen::NyarTranslator::new();
        translator
            .generate(&egraph, root_id)
            .map_err(|e| format!("Codegen error: {:?}", e))
    }
}

impl NyarFrontend for MiniTypescriptFrontend {
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

    fn lower(&self, ast: &TypeScriptRoot) -> Result<IKunTree, NyarError> {
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
                self.builder.assign(&var.name, value, loc)
            }
            ast::Statement::FunctionDeclaration(func) => {
                let loc = self.to_loc(func.span.clone().into());
                let mut body_ids = Vec::new();
                for s in func.body {
                    body_ids.push(self.convert_statement(s));
                }
                let lambda = self.builder.function(&func.name, func.params, body_ids);
                self.builder.assign(&func.name, lambda, loc)
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
                if let Some(ext) = class.extends {
                    args.push(self.builder.symbol(&ext, loc.clone()));
                } else {
                    args.push(self.builder.constant(0, loc.clone())); // No base class
                }

                for member in class.body {
                    match member {
                        ast::ClassMember::Property {
                            name,
                            ty,
                            initializer,
                            span,
                        } => {
                            let mloc = self.to_loc(span.into());
                            let init_id = if let Some(expr) = initializer {
                                self.convert_expression(expr)
                            } else {
                                self.builder.constant(0, mloc.clone())
                            };
                            let ty_id = if let Some(t) = ty {
                                self.builder.symbol(&t, mloc.clone())
                            } else {
                                self.builder.symbol("any", mloc.clone())
                            };
                            let field_name = self.builder.symbol(&name, mloc.clone());
                            args.push(self.builder.extension(
                                "gc.field",
                                vec![field_name, ty_id, init_id],
                                mloc,
                            ));
                        }
                        ast::ClassMember::Method {
                            name,
                            params,
                            body,
                            span,
                        } => {
                            let mloc = self.to_loc(span.into());
                            let mut body_ids = Vec::new();
                            for s in body {
                                body_ids.push(self.convert_statement(s));
                            }
                            let lambda = self.builder.function(&name, params, body_ids);
                            let method_name = self.builder.symbol(&name, mloc.clone());
                            args.push(self.builder.extension(
                                "gc.method",
                                vec![method_name, lambda],
                                mloc,
                            ));
                        }
                    }
                }
                self.builder.extension("gc.struct", args, loc)
            }
        }
    }

    fn convert_expression(&mut self, expr: ast::Expression) -> Id {
        match expr {
            ast::Expression::Identifier(name) => self.builder.symbol(&name, Loc::default()),
            ast::Expression::NumericLiteral(val) => {
                self.builder.constant(val as i64, Loc::default())
            }
            ast::Expression::StringLiteral(val) => self.builder.string(&val, Loc::default()),
            ast::Expression::BooleanLiteral(val) => self.builder.bool(val, Loc::default()),
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
                                        "std::io::println",
                                        arg_ids,
                                        loc,
                                    );
                                } else if prop_name == "print" {
                                    return self.builder.cross_lang_call(
                                        "nyar",
                                        "std::io::print",
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
                ..
            } => {
                let obj = self.convert_expression(*object);
                let prop = self.convert_expression(*property);
                if computed {
                    self.builder
                        .extension("index", vec![obj, prop], Loc::default())
                } else {
                    self.builder
                        .extension("gc.get_field", vec![obj, prop], Loc::default())
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
            _ => self.builder.constant(0, Loc::default()),
        }
    }
}
