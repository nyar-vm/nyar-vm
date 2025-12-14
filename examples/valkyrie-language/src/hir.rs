use crate::ast::{Expr, Pattern, Stmt};
use crate::lexer::Error;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct Scope {
    pub namespace: Vec<String>,
    pub imports: Vec<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct HIRModule {
    pub namespace_stack: Vec<String>,
    pub use_prefixes: Vec<Vec<String>>,
    pub functions: Vec<HirFunc>,
    pub classes: Vec<HirClass>,
    pub enums: Vec<Stmt>,
    pub traits: Vec<HirTrait>,
    pub impls: Vec<HirImpl>,
    pub main: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub struct HirFunc {
    pub name: String,
    pub args: Vec<String>,
    pub body: Vec<Stmt>,
    pub scope: Scope,
}

impl HirFunc {
    pub fn new(name: String, args: Vec<String>, body: Vec<Stmt>, scope: Scope) -> Self {
        Self {
            name,
            args,
            body,
            scope,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HirClass {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct HirTrait {
    pub name: String,
    pub methods: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct HirImpl {
    pub trait_name: String,
    pub class_name: String,
    pub methods: Vec<HirFunc>,
}

fn def_name(namespace_stack: &[String], name: &str) -> String {
    if name.contains("::") {
        name.to_string()
    } else if namespace_stack.is_empty() {
        name.to_string()
    } else {
        format!("{}::{name}", namespace_stack.join("::"))
    }
}

pub fn build_hir(stmts: &[Stmt]) -> Result<HIRModule, Error> {
    let mut namespace_stack: Vec<String> = Vec::new();
    let mut use_prefixes: Vec<Vec<String>> = Vec::new();

    let mut functions: Vec<HirFunc> = Vec::new();
    let mut classes: Vec<HirClass> = Vec::new();
    let mut enums: Vec<Stmt> = Vec::new();
    let mut traits: Vec<HirTrait> = Vec::new();
    let mut impls: Vec<HirImpl> = Vec::new();
    let mut main: Vec<Stmt> = Vec::new();

    fn push_block(
        namespace_stack: &mut Vec<String>,
        use_prefixes: &mut Vec<Vec<String>>,
        functions: &mut Vec<HirFunc>,
        classes: &mut Vec<HirClass>,
        enums: &mut Vec<Stmt>,
        traits: &mut Vec<HirTrait>,
        impls: &mut Vec<HirImpl>,
        main: &mut Vec<Stmt>,
        body: &[Stmt],
    ) -> Result<(), Error> {
        let base_uses = use_prefixes.len();
        for stmt in body {
            match stmt {
                Stmt::NamespaceSet(path) => {
                    *namespace_stack = path.clone();
                }
                Stmt::NamespaceDef(name, inner) => {
                    namespace_stack.push(name.clone());
                    push_block(
                        namespace_stack,
                        use_prefixes,
                        functions,
                        classes,
                        enums,
                        traits,
                        impls,
                        main,
                        inner,
                    )?;
                    namespace_stack.pop();
                    use_prefixes.truncate(base_uses);
                }
                Stmt::Using(path) => {
                    use_prefixes.push(path.clone());
                }
                Stmt::FuncDef(name, args, body) => {
                    let qname = def_name(namespace_stack, name);
                    functions.push(HirFunc::new(
                        qname,
                        args.clone(),
                        body.clone(),
                        Scope {
                            namespace: namespace_stack.clone(),
                            imports: use_prefixes.clone(),
                        },
                    ));
                }
                Stmt::ClassDef(name, fields) => {
                    let qname = def_name(namespace_stack, name);
                    classes.push(HirClass {
                        name: qname,
                        fields: fields.clone(),
                    });
                }
                Stmt::TraitDef(name, methods) => {
                    let qname = def_name(namespace_stack, name);
                    traits.push(HirTrait {
                        name: qname,
                        methods: methods.clone(),
                    });
                }
                Stmt::EnumDef(name, variants) => {
                    let qname = def_name(namespace_stack, name);
                    println!("DEBUG: HIR pushing enum {}", qname);
                    enums.push(Stmt::EnumDef(qname, variants.clone()));
                }
                Stmt::ImplDef(trait_name, class_name, methods) => {
                    let mut mfuncs = Vec::new();
                    for m in methods {
                        match m {
                            Stmt::FuncDef(name, args, body) => {
                                let qname = def_name(namespace_stack, name);
                                mfuncs.push(HirFunc::new(
                                    qname,
                                    args.clone(),
                                    body.clone(),
                                    Scope {
                                        namespace: namespace_stack.clone(),
                                        imports: use_prefixes.clone(),
                                    },
                                ));
                            }
                            _ => {
                                return Err(Error::Compile(
                                    "impl block can only contain function definitions".into(),
                                ))
                            }
                        }
                    }
                    impls.push(HirImpl {
                        trait_name: trait_name.clone(),
                        class_name: class_name.clone(),
                        methods: mfuncs,
                    });
                }
                Stmt::ImplyDef(class_name, methods) => {
                    for m in methods {
                        if let Stmt::FuncDef(name, args, body) = m {
                            let qname = format!("{class_name}::{name}");
                            let qname = def_name(namespace_stack, &qname);
                            functions.push(HirFunc::new(
                                qname,
                                args.clone(),
                                body.clone(),
                                Scope {
                                    namespace: namespace_stack.clone(),
                                    imports: use_prefixes.clone(),
                                },
                            ));
                        } else {
                            return Err(Error::Compile(
                                "imply block can only contain function definitions".into(),
                            ));
                        }
                    }
                }
                _ => {
                    // Other executable statements go to main
                    println!("DEBUG: HIR main pushing stmt: {:?}", stmt);
                    main.push(stmt.clone());
                }
            }
        }
        Ok(())
    }

    push_block(
        &mut namespace_stack,
        &mut use_prefixes,
        &mut functions,
        &mut classes,
        &mut enums,
        &mut traits,
        &mut impls,
        &mut main,
        stmts,
    )?;

    let mut module = HIRModule {
        namespace_stack,
        use_prefixes,
        functions,
        classes,
        enums,
        traits,
        impls,
        main,
    };
    
    resolve_hir(&mut module)?;
    
    Ok(module)
}

fn resolve_hir(module: &mut HIRModule) -> Result<(), Error> {
    let mut definitions = HashSet::new();
    
    // Collect all definitions
    for f in &module.functions {
        definitions.insert(f.name.clone());
    }
    for c in &module.classes {
        definitions.insert(c.name.clone());
    }
    for e in &module.enums {
        if let Stmt::EnumDef(name, variants) = e {
            definitions.insert(name.clone());
            for (v_name, _) in variants {
                let v_key = format!("{}::{}", name, v_name);
                definitions.insert(v_key);
            }
        }
    }
    
    // Helper to resolve a name
    let resolve_name = |name: &str, scope: &Scope| -> Option<String> {
        if definitions.contains(name) {
            return Some(name.to_string());
        }
        // Try namespace
        if !scope.namespace.is_empty() {
            let q = format!("{}::{}", scope.namespace.join("::"), name);
            if definitions.contains(&q) {
                return Some(q);
            }
        }
        // Try imports
        for imp in &scope.imports {
             let q = format!("{}::{}", imp.join("::"), name);
             if definitions.contains(&q) {
                 return Some(q);
             }
        }
        None
    };

    // Recursive resolution functions
    fn resolve_pattern(
        pat: &mut Pattern,
        resolve: &dyn Fn(&str) -> Option<String>,
    ) {
        match pat {
            Pattern::Constructor(name, args) => {
                if let Some(q) = resolve(name) {
                    *name = q;
                }
                for arg in args {
                    resolve_pattern(arg, resolve);
                }
            }
            // Pattern::Variable is usually binding, but could be unit variant?
            // If it matches a defined name, it's a constant pattern/unit variant.
            // But parser distinguishes based on capitalization.
            // If it is uppercase, parser made it Constructor.
            // If it is lowercase, parser made it Variable.
            // But TokenKind::EOF is uppercase.
            // So Constructor case handles it.
            _ => {}
        }
    }

    fn resolve_expr(
        expr: &mut Expr,
        resolve: &dyn Fn(&str) -> Option<String>,
    ) {
        match expr {
            Expr::Variable(name) => {
                if let Some(q) = resolve(name) {
                    *name = q;
                }
            }
            Expr::New(name) => {
                if let Some(q) = resolve(name) {
                    *name = q;
                }
            }
            Expr::InstanceOf(e, name) => {
                resolve_expr(e, resolve);
                if let Some(q) = resolve(name) {
                    *name = q;
                }
            }
            Expr::Cast(e, name) => {
                resolve_expr(e, resolve);
                if let Some(q) = resolve(name) {
                    *name = q;
                }
            }
            Expr::CheckCast(e, name) => {
                resolve_expr(e, resolve);
                if let Some(q) = resolve(name) {
                    *name = q;
                }
            }
            Expr::Call(callee, args) => {
                resolve_expr(callee, resolve);
                for arg in args {
                    resolve_expr(arg, resolve);
                }
            }
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) |
            Expr::And(a, b) | Expr::Or(a, b) | Expr::Eq(a, b) | Expr::Ne(a, b) |
            Expr::Lt(a, b) | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) => {
                resolve_expr(a, resolve);
                resolve_expr(b, resolve);
            }
            Expr::Not(e) | Expr::Neg(e) | Expr::TypeOf(e) => {
                resolve_expr(e, resolve);
            }
            Expr::GetField(e, _) => {
                resolve_expr(e, resolve);
            }
            Expr::SetField(e, _, v) => {
                resolve_expr(e, resolve);
                resolve_expr(v, resolve);
            }
            Expr::SetLocal(_, v) => {
                resolve_expr(v, resolve);
            }
            Expr::Closure(_, body) => {
                // Closure has its own scope? Or inherits?
                // Inherits.
                 for stmt in body {
                    resolve_stmt(stmt, resolve);
                }
            }
            Expr::Match(target, branches) => {
                resolve_expr(target, resolve);
                for (pat, body) in branches {
                    resolve_pattern(pat, resolve);
                     for stmt in body {
                        resolve_stmt(stmt, resolve);
                    }
                }
            }
            _ => {}
        }
    }

    fn resolve_stmt(
        stmt: &mut Stmt,
        resolve: &dyn Fn(&str) -> Option<String>,
    ) {
        match stmt {
            Stmt::Expr(e) => resolve_expr(e, resolve),
            Stmt::Let(_, e) => resolve_expr(e, resolve),
            Stmt::Return(e) => resolve_expr(e, resolve),
            Stmt::Yield(e) => resolve_expr(e, resolve),
            Stmt::Assert(e, msg) => {
                resolve_expr(e, resolve);
                if let Some(m) = msg {
                    resolve_expr(m, resolve);
                }
            }
            Stmt::Debug(e) => resolve_expr(e, resolve),
            Stmt::If(c, t, e) => {
                resolve_expr(c, resolve);
                for s in t {
                    resolve_stmt(s, resolve);
                }
                if let Some(el) = e {
                     for s in el {
                        resolve_stmt(s, resolve);
                    }
                }
            }
            Stmt::While(c, b) => {
                resolve_expr(c, resolve);
                 for s in b {
                    resolve_stmt(s, resolve);
                }
            }
            Stmt::Loop(b) => {
                 for s in b {
                    resolve_stmt(s, resolve);
                }
            }
            // FuncDef inside body? (Closure-like or inner function)
            // They are not HirFunc, they are Stmt::FuncDef.
            // They should also be resolved using CURRENT scope.
            Stmt::FuncDef(_, _, body) => {
                 for s in body {
                    resolve_stmt(s, resolve);
                }
            }
            _ => {}
        }
    }

    // Resolve all functions
    for f in &mut module.functions {
        let resolve = |name: &str| resolve_name(name, &f.scope);
        for stmt in &mut f.body {
            resolve_stmt(stmt, &resolve);
        }
    }
    
    // Resolve impl methods
    for im in &mut module.impls {
         for f in &mut im.methods {
            let resolve = |name: &str| resolve_name(name, &f.scope);
            for stmt in &mut f.body {
                resolve_stmt(stmt, &resolve);
            }
        }
    }

    Ok(())
}
