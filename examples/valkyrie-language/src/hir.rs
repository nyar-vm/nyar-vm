use crate::ast::Stmt;
use crate::lexer::Error;

#[derive(Clone, Debug)]
pub struct HIRModule {
    pub namespace_stack: Vec<String>,
    pub use_prefixes: Vec<Vec<String>>,
    pub functions: Vec<HirFunc>,
    pub classes: Vec<HirClass>,
    pub traits: Vec<HirTrait>,
    pub impls: Vec<HirImpl>,
    pub main: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub struct HirFunc {
    pub name: String,
    pub args: Vec<String>,
    pub body: Vec<Stmt>,
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
    let mut traits: Vec<HirTrait> = Vec::new();
    let mut impls: Vec<HirImpl> = Vec::new();
    let mut main: Vec<Stmt> = Vec::new();

    fn push_block(
        namespace_stack: &mut Vec<String>,
        use_prefixes: &mut Vec<Vec<String>>,
        functions: &mut Vec<HirFunc>,
        classes: &mut Vec<HirClass>,
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
                    functions.push(HirFunc {
                        name: qname,
                        args: args.clone(),
                        body: body.clone(),
                    });
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
                Stmt::EnumDef(_, _) => {
                    // Leave enums to lowering and emission; keep original stmt in main
                    main.push(stmt.clone());
                }
                Stmt::ImplDef(trait_name, class_name, methods) => {
                    let mut mfuncs = Vec::new();
                    for m in methods {
                        match m {
                            Stmt::FuncDef(name, args, body) => {
                                let qname = def_name(namespace_stack, name);
                                mfuncs.push(HirFunc {
                                    name: qname,
                                    args: args.clone(),
                                    body: body.clone(),
                                });
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
                            functions.push(HirFunc {
                                name: qname,
                                args: args.clone(),
                                body: body.clone(),
                            });
                        } else {
                            return Err(Error::Compile(
                                "imply block can only contain function definitions".into(),
                            ));
                        }
                    }
                }
                _ => {
                    // Other executable statements go to main
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
        &mut traits,
        &mut impls,
        &mut main,
        stmts,
    )?;

    Ok(HIRModule {
        namespace_stack,
        use_prefixes,
        functions,
        classes,
        traits,
        impls,
        main,
    })
}
