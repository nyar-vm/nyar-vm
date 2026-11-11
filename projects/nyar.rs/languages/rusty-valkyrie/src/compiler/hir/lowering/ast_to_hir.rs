use crate::compiler::{
    hir::{
        GenericBinderScope, NominalTypeScope, bind_lambda_environments, hoist_anonymous_classes, hoist_lambdas,
        overload::{resolve_hir_calls, validate_extractor_patterns},
    },
    mir::{SINGLETON_CONSTRUCTOR_NAME, SINGLETON_FINALIZER_NAME},
    semantic::operator_members,
    syntax::oak::{
        self as frontend,
        ast::{
            AssociatedType, Attribute, ClassDeclaration, EnumVariant, Enums, EnumsKind, FieldDeclaration, Flags, GenericParam,
            ImplyDeclaration, MethodDeclaration, MicroDeclaration, NamePath as AstNamePath, Param, Parent, SingletonDeclaration, StatementNode,
            StructureDeclaration, TermExpression, Trait, TypeAliasDeclaration, TypeExpression, TypeFunction, UnionDeclaration,
            UsingDeclaration, Variant, WidgetDeclaration,
        },
    },
    types::{
        Identifier, NamePath, SourceID,
        hir::{
            HIR_WIRE_VERSION, HirAssociatedTypeImpl, HirAttribute, HirBlock, HirCompileWarning, HirDependencySemanticExport,
            HirDocumentation, HirEnum, HirExpr, HirExprKind, HirField, HirFileScope, HirFlagMember, HirFlags, HirFunction, HirImpl, HirImport,
            HirImportBinding, HirKind, HirLiteral, HirModule, HirParam, HirParameterBindingKind, HirSingleton, HirStatement,
            HirStatementKind, HirStruct, HirTrait, HirTraitBound, HirTypeAlias, HirTypeFunction, HirVariadicKind, HirVariant, HirVisibility,
            HirWhereConstraint, HirWidget, HirWidgetLifecycle, ValkyrieType,
        },
    },
};

use crate::compiler::hir::lowering::{
    diagnostics::{CompileWarningScope, plan_source, plan_span, take_compile_warnings},
    errors::HirError,
    generic_binders::{freeze_module_generic_binders, register_field_declarations, register_function_declarations, synthesize_parent_fields},
    nominal_declarations::validate_enum_discriminators,
    source_group::{
        register_root_type_alias_targets, root_type_alias_declarations, validate_source_groups, visible_nominal_type_names,
        visible_type_aliases,
    },
    trait_binding::bind_trait_declarations,
};

use crate::compiler::hir::lowering::{
    attribute_lowering::*,
    expr_lowering::{extract_name_path, lower_block, lower_term_expression},
};

/// Lowers `ValkyrieRoot` into `HirModule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AstToHir {
    /// Source id attached to lowered items.
    pub source_id: SourceID,
}

impl AstToHir {
    /// Creates a new lowerer bound to a source id.
    pub fn new(source_id: SourceID) -> Self {
        Self { source_id }
    }

    /// Lowers a parser root into a module-shaped HIR view.
    pub fn lower_root(&self, root: &crate::compiler::syntax::oak::ValkyrieRoot) -> Result<HirModule, HirError> {
        self.lower_root_with_semantic_exports(root, &[])
    }

    /// Lowers a parser root with resolved nominal dependency exports in scope.
    pub fn lower_root_with_semantic_exports(
        &self,
        root: &crate::compiler::syntax::oak::ValkyrieRoot,
        imported_semantic_exports: &[HirDependencySemanticExport],
    ) -> Result<HirModule, HirError> {
        self.lower_root_with_semantic_exports_and_name(root, imported_semantic_exports, None)
    }

    /// 在 HIR 调用解析前覆盖模块身份；仅供完整 source closure 编译使用。
    pub fn lower_root_with_semantic_exports_and_name(
        &self,
        root: &crate::compiler::syntax::oak::ValkyrieRoot,
        imported_semantic_exports: &[HirDependencySemanticExport],
        module_name_override: Option<NamePath>,
    ) -> Result<HirModule, HirError> {
        let mut hir = self.lower_root_without_call_resolution(root, imported_semantic_exports, module_name_override)?;
        if !imported_semantic_exports.is_empty() {
            return Err(HirError::DependencyDeclarationsRequireSourceClosure {
                imported_export_count: imported_semantic_exports.len(),
                source: plan_source(),
                span: plan_span(),
            });
        }
        register_function_declarations(&mut hir, &mut 0, &mut 0)?;
        // 父类槽位必须在字段身份登记之前合成：它和声明体字段共用同一 `FieldId` 空间与冻结路径。
        synthesize_parent_fields(&mut hir)?;
        register_field_declarations(&mut hir, &mut 0)?;
        // 环境身份必须在 binder 冻结之前回填：环境聚合的实参先落 `Named` 占位，
        // 由随后的 `freeze_module_generic_binders` 用外层 callable 的 bearer 冻结，
        // 单态化才能按具体实参代入环境类型。
        bind_lambda_environments(&mut hir);
        // 先冻结声明身份（含 impl target 的名义身份），再统一冻结泛型 binder：
        // impl 成员的 owner binder 只有 target 已解析后才能按声明写定。
        crate::compiler::hir::lowering::generic_binders::bind_declared_nominal_types(&mut hir)?;
        freeze_module_generic_binders(&mut hir)?;
        bind_trait_declarations(&mut hir)?;
        // 集合字面量转换跑在 trait 绑定之后、调用解析之前：
        // 字面量项仍需 `resolve_hir_calls` 解析调用，而 `CollectionInit` 两槽的
        // 声明身份已冻结、实例留空，交由随后的单态化固定点自然铸造。
        crate::compiler::hir::lowering::collection_init::transform_collection_literals(&mut hir)?;
        // 插值脱糖与集合字面量同阶段：都在调用解析之前把语法糖改写成普通调用，
        // 新增的 `display` / `infix +` 调用由随后的统一解析冻结身份。
        crate::compiler::hir::lowering::interpolation::desugar_string_interpolations(&mut hir)?;
        resolve_hir_calls(&mut hir);
        // 调用合同冻结后立即执行 HIR 唯一定型：引号捕获在进单态化与 MIR 之前
        // 全部结算为最终字面量形态（§4.10 B / §4.12 前端闭合门）。
        crate::compiler::hir::lowering::settle_quote_literal_captures(&mut hir)?;
        let mut instantiation_state = crate::compiler::hir::lowering::GenericInstantiationState::default();
        crate::compiler::hir::lowering::instantiate_generic_callables(
            std::slice::from_mut(&mut hir),
            None,
            &std::collections::BTreeMap::new(),
            &mut instantiation_state,
        )?;
        validate_extractor_patterns(&hir)?;
        Ok(hir)
    }

    /// 仅物化 HIR；调用身份必须由完整源码组阶段统一解析。
    pub(crate) fn lower_root_without_call_resolution(
        &self,
        root: &crate::compiler::syntax::oak::ValkyrieRoot,
        imported_semantic_exports: &[HirDependencySemanticExport],
        module_name_override: Option<NamePath>,
    ) -> Result<HirModule, HirError> {
        let _type_alias_scope =
            crate::compiler::hir::ModuleTypeAliasScope::enter(visible_type_aliases(&[root], imported_semantic_exports), false);
        let _nominal_scope = NominalTypeScope::enter(visible_nominal_type_names(&[root], imported_semantic_exports));
        register_root_type_alias_targets(&[root])?;
        self.lower_prepared_root(root, imported_semantic_exports, module_name_override)
    }

    /// 降低一个已完成宏/模板展开、且别名作用域已经建立的源码根。
    pub(crate) fn lower_prepared_root(
        &self,
        root: &crate::compiler::syntax::oak::ValkyrieRoot,
        imported_semantic_exports: &[HirDependencySemanticExport],
        module_name_override: Option<NamePath>,
    ) -> Result<HirModule, HirError> {
        crate::compiler::hir::validate_ast_root(root)?;
        let _warning_scope = CompileWarningScope::enter();
        let module_name = module_name_override.unwrap_or_else(|| {
            root.items
                .iter()
                .find_map(|item| match item {
                    StatementNode::Namespace(namespace) if crate::compiler::syntax::oak::namespace_is_declaration_only(namespace) => {
                        Some(lower_name_path(&namespace.name))
                    }
                    _ => None,
                })
                .unwrap_or_else(default_module_name)
        });

        // 导入必须先经过 fail-closed 校验：真正缺 path 的畸形 `using` 不得被翻译成空
        // `HirImport`。`using!` 是主空间一等导入，只校验放置，不走空 path 拒绝路径。
        // 文件局部 `using` 与主空间 `using!` 分表保存：前者进文件作用域，后者进 unit 共享表。
        let mut imports = Vec::new();
        let mut primary_imports = Vec::new();
        let mut primary_namespace = false;
        let mut primary_scope = false;
        for item in &root.items {
            match item {
                StatementNode::Namespace(namespace) if crate::compiler::syntax::oak::namespace_is_declaration_only(namespace) => {
                    primary_scope = namespace_is_primary(namespace);
                    if namespace_is_primary(namespace) {
                        if primary_namespace {
                            return Err(HirError::DuplicatePrimaryNamespace {
                                namespaces: vec![lower_namespace_name(namespace)],
                                source: nyar_types::DiagnosticSource { id: "<ast>".into(), revision: None },
                                span: nyar_types::DiagnosticSpan { start: namespace.span.start, end: namespace.span.end },
                            });
                        }
                        primary_namespace = true;
                    }
                }
                StatementNode::Using(using) => {
                    if using_is_primary(using) && !primary_scope {
                        return Err(HirError::PrimaryImportOutsidePrimaryNamespace {
                            source: nyar_types::DiagnosticSource { id: "<ast>".into(), revision: None },
                            span: nyar_types::DiagnosticSpan { start: using.span.start, end: using.span.end },
                        });
                    }
                    let lowered = lower_using(using)?;
                    if using_is_primary(using) {
                        primary_imports.push(lowered);
                    }
                    else {
                        imports.push(lowered);
                    }
                }
                StatementNode::Namespace(namespace) if namespace.body.is_some() => {
                    if namespace_is_primary(namespace) {
                        if primary_namespace {
                            return Err(HirError::DuplicatePrimaryNamespace {
                                namespaces: vec![lower_namespace_name(namespace)],
                                source: nyar_types::DiagnosticSource { id: "<ast>".into(), revision: None },
                                span: nyar_types::DiagnosticSpan { start: namespace.span.start, end: namespace.span.end },
                            });
                        }
                        primary_namespace = true;
                    }
                    lower_namespace_body_imports(
                        crate::compiler::syntax::oak::namespace_items(namespace),
                        namespace_is_primary(namespace),
                        &mut imports,
                        &mut primary_imports,
                    )?;
                }
                _ => {}
            }
        }

        let file_namespace = root
            .items
            .iter()
            .find_map(|item| match item {
                StatementNode::Namespace(namespace) if crate::compiler::syntax::oak::namespace_is_declaration_only(namespace) => {
                    Some(lower_name_path(&namespace.name).parts().to_vec())
                }
                _ => None,
            })
            .unwrap_or_default();

        let mut type_aliases = Vec::new();

        let mut functions = Vec::new();
        let mut type_functions = Vec::new();
        let mut structs = Vec::new();
        let mut widgets = Vec::new();
        let mut singletons = Vec::new();
        let mut traits = Vec::new();
        let mut enums = Vec::new();
        let mut flags = Vec::new();
        let mut impls = Vec::new();

        let mut current_namespace = NamePath::default();
        for item in &root.items {
            match item {
                StatementNode::Namespace(namespace) => {
                    if crate::compiler::syntax::oak::namespace_is_declaration_only(namespace) {
                        current_namespace = lower_name_path(&namespace.name);
                    }
                    else {
                        let namespace_path = lower_name_path(&namespace.name);
                        self.lower_items(
                            crate::compiler::syntax::oak::namespace_items(namespace),
                            &namespace_path,
                            &mut functions,
                            &mut type_functions,
                            &mut structs,
                            &mut widgets,
                            &mut singletons,
                            &mut traits,
                            &mut enums,
                            &mut flags,
                            &mut impls,
                            &mut type_aliases,
                        )?;
                    }
                }
                other => self.lower_statement_node(
                    other,
                    &current_namespace,
                    &mut functions,
                    &mut type_functions,
                    &mut structs,
                    &mut widgets,
                    &mut singletons,
                    &mut traits,
                    &mut enums,
                    &mut flags,
                    &mut impls,
                    &mut type_aliases,
                )?,
            }
        }

        let mut hir = HirModule {
            name: module_name,
            wire_version: HIR_WIRE_VERSION,
            dependency_key: String::new(),
            source_path: String::new(),
            primary_namespace,
            doc: HirDocumentation::default(),
            files: vec![HirFileScope {
                path: String::new(),
                source_id: self.source_id.version_id,
                namespace: file_namespace,
                imports,
                declarations: Vec::new(),
            }],
            primary_imports,
            warnings: Vec::new(),
            submodules: Vec::new(),
            functions,
            structs,
            enums,
            imported_enums: Vec::new(),
            imported_semantic_exports: imported_semantic_exports.to_vec(),
            flags,
            traits,
            impls,
            type_functions,
            type_families: Vec::new(),
            widgets,
            singletons,
            statements: Vec::new(),
            type_aliases,
            instantiated_callables: Vec::new(),
            diagnostic_sources: std::collections::BTreeMap::new(),
            call_resolution_failures: Vec::new(),
        };
        hoist_anonymous_classes(&mut hir);
        hoist_lambdas(&mut hir);
        let mut injector = crate::compiler::expansion::derive::DeriveInjector::new();
        let derive_result = injector.inject_derives(&mut hir);
        if derive_result.has_errors() {
            return Err(HirError::DeriveResolutionFailed { errors: derive_result.errors });
        }
        hir.warnings = take_compile_warnings();
        hir.files[0].declarations = Self::collect_file_declarations(&hir);
        Ok(hir)
    }

    /// 收集本模块（含嵌套子模块）声明面的本地名，供文件作用域记录。
    fn collect_file_declarations(module: &HirModule) -> Vec<Identifier> {
        let mut names = Vec::new();
        Self::collect_declared_names(module, &mut names);
        names.sort();
        names.dedup();
        names
    }

    fn collect_declared_names(module: &HirModule, names: &mut Vec<Identifier>) {
        names.extend(module.functions.iter().map(|function| function.name.clone()));
        names.extend(module.structs.iter().map(|item| item.name.clone()));
        names.extend(module.enums.iter().map(|item| item.name.clone()));
        names.extend(module.traits.iter().map(|item| item.name.clone()));
        names.extend(module.type_aliases.iter().map(|item| item.name.clone()));
        for submodule in &module.submodules {
            Self::collect_declared_names(submodule, names);
        }
    }

    fn lower_items(
        &self,
        items: &[StatementNode],
        namespace: &NamePath,
        functions: &mut Vec<HirFunction>,
        type_functions: &mut Vec<HirTypeFunction>,
        structs: &mut Vec<HirStruct>,
        widgets: &mut Vec<HirWidget>,
        singletons: &mut Vec<HirSingleton>,
        traits: &mut Vec<HirTrait>,
        enums: &mut Vec<HirEnum>,
        flags: &mut Vec<HirFlags>,
        impls: &mut Vec<HirImpl>,
        type_aliases: &mut Vec<HirTypeAlias>,
    ) -> Result<(), HirError> {
        for item in items {
            self.lower_statement_node(
                item,
                namespace,
                functions,
                type_functions,
                structs,
                widgets,
                singletons,
                traits,
                enums,
                flags,
                impls,
                type_aliases,
            )?;
        }
        Ok(())
    }

    fn lower_statement_node(
        &self,
        item: &StatementNode,
        namespace: &NamePath,
        functions: &mut Vec<HirFunction>,
        type_functions: &mut Vec<HirTypeFunction>,
        structs: &mut Vec<HirStruct>,
        widgets: &mut Vec<HirWidget>,
        singletons: &mut Vec<HirSingleton>,
        traits: &mut Vec<HirTrait>,
        enums: &mut Vec<HirEnum>,
        flags: &mut Vec<HirFlags>,
        impls: &mut Vec<HirImpl>,
        type_aliases: &mut Vec<HirTypeAlias>,
    ) -> Result<(), HirError> {
        match item {
            StatementNode::Micro(micro) => {
                functions.push(self.lower_micro(micro, namespace)?);
                Ok(())
            }
            StatementNode::TypeFunction(type_function) => {
                type_functions.push(self.lower_type_function(type_function)?);
                Ok(())
            }
            StatementNode::TypeAlias(alias) => {
                type_aliases.push(self.lower_type_alias(alias, namespace)?);
                Ok(())
            }
            StatementNode::Class(class_decl) => {
                structs.push(self.lower_class(class_decl, namespace)?);
                Ok(())
            }
            StatementNode::Structure(structure) => {
                structs.push(self.lower_structure(structure, namespace)?);
                Ok(())
            }
            StatementNode::Singleton(singleton) => {
                singletons.push(self.lower_singleton(singleton, namespace)?);
                Ok(())
            }
            StatementNode::Widget(widget) => {
                widgets.push(self.lower_widget(widget)?);
                Ok(())
            }
            StatementNode::Trait(trait_decl) => {
                let mut lowered = self.lower_trait(trait_decl)?;
                lowered.declaring_namespace = namespace.clone();
                traits.push(lowered);
                Ok(())
            }
            StatementNode::Enums(enum_decl) => {
                enums.push(self.lower_enums(enum_decl)?);
                Ok(())
            }
            StatementNode::Variant(variant_decl) => {
                enums.push(self.lower_variant_decl(variant_decl)?);
                Ok(())
            }
            StatementNode::Flags(flags_decl) => {
                flags.push(self.lower_flags(flags_decl)?);
                Ok(())
            }
            StatementNode::Imply(imply) => {
                impls.push(self.lower_imply(imply, namespace)?);
                Ok(())
            }
            StatementNode::Union(union_decl) => {
                Err(HirError::UnionNotLowered { union_name: union_decl.name.name.clone(), source: plan_source(), span: plan_span() })
            }
            StatementNode::Using(_)
            | StatementNode::Namespace(_)
            | StatementNode::Let(_)
            | StatementNode::ExprStmt(_)
            | StatementNode::Statement(_) => Ok(()),
            StatementNode::Template(_) => Err(HirError::UnexpandedTemplateItem { source: plan_source(), span: plan_span() }),
            unsupported => Err(HirError::UnsupportedRootItem {
                item_kind: crate::compiler::hir::lowering::template_expand::statement_node_kind(unsupported).to_string(),
                source: plan_source(),
                span: plan_span(),
            }),
        }
    }

    fn lower_micro(&self, function: &MicroDeclaration, declaring_namespace: &NamePath) -> Result<HirFunction, HirError> {
        let _generic_scope = GenericBinderScope::enter(&generic_parameter_names(&function.generics), &[]);
        let span = frontend::std_range(&function.span);
        let annotations = function
            .annotations
            .iter()
            .map(|attribute| lower_attribute(attribute, self.source_id, span.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let body = lower_block(&function.body, self.source_id, span.clone())?;
        let declares_external_import = body.statements.is_empty()
            && body.expr.is_none()
            && crate::compiler::emitter::backend_contract::interop::annotations_declare_external_import(&annotations);
        Ok(HirFunction {
            declaration: None,
            instance: None,
            name: operator_member_name(&function.name, &function.annotations),
            declaring_namespace: declaring_namespace.clone(),
            doc: lower_documentation(&function.annotations),
            annotations,
            generics: lower_generic_parameters(&function.generics),
            where_constraints: lower_where_constraints(&function.where_constraints, self.source_id)?,
            params: function.params.iter().map(|param| lower_param(param, self.source_id, span.clone())).collect::<Result<Vec<_>, _>>()?,
            return_type: function
                .return_type
                .as_ref()
                .map(crate::compiler::hir::lower_type_expression)
                .transpose()?
                .unwrap_or(ValkyrieType::Unit),
            body,
            span: with_source(&span, self.source_id),
            visibility: lower_visibility(&function.annotations),
            is_abstract: function.is_abstract || has_modifier(&function.annotations, "abstract") || declares_external_import,
            is_final: has_modifier(&function.annotations, "final"),
            is_virtual: false,
            is_override: false,
        })
    }

    fn lower_type_alias(&self, alias: &TypeAliasDeclaration, namespace: &NamePath) -> Result<HirTypeAlias, HirError> {
        Ok(HirTypeAlias {
            name: Identifier::new(&alias.name.name),
            namespace: namespace.parts().to_vec(),
            generics: Vec::new(),
            target: crate::compiler::hir::lower_type_expression(&alias.target)?,
            span: with_source(&frontend::std_range(&alias.span), self.source_id),
        })
    }

    fn lower_type_function(&self, function: &TypeFunction) -> Result<HirTypeFunction, HirError> {
        let span = frontend::std_range(&function.span);
        let _generic_scope = GenericBinderScope::enter(&generic_parameter_names(&function.generics), &[]);
        Ok(HirTypeFunction {
            name: Identifier::new(&function.name.name),
            documents: lower_documentation(&function.annotations),
            generics: lower_generic_parameters(&function.generics),
            where_constraints: lower_where_constraints(&function.where_constraints, self.source_id)?,
            params: function.params.iter().map(|param| lower_param(param, self.source_id, span.clone())).collect::<Result<Vec<_>, _>>()?,
            return_type: function
                .return_type
                .as_ref()
                .map(crate::compiler::hir::lower_type_expression)
                .transpose()?
                .unwrap_or(ValkyrieType::Unit),
            body: lower_block(&function.body, self.source_id, span)?,
        })
    }

    fn lower_class(&self, class_decl: &ClassDeclaration, namespace: &NamePath) -> Result<HirStruct, HirError> {
        let _generic_scope = GenericBinderScope::enter(&generic_parameter_names(&class_decl.generics), &[]);
        let span = frontend::std_range(&class_decl.span);
        let hir_annotations = lower_attributes(&class_decl.annotations, self.source_id, span)?;
        self.lower_class_like(
            Identifier::new(&class_decl.name.name),
            namespace.parts(),
            &class_decl.annotations,
            hir_annotations,
            &class_decl.generics,
            &class_decl.parents,
            &class_decl.body.fields,
            &class_decl.body.methods,
            false,
        )
    }

    fn lower_structure(&self, structure: &StructureDeclaration, namespace: &NamePath) -> Result<HirStruct, HirError> {
        let _generic_scope = GenericBinderScope::enter(&generic_parameter_names(&structure.generics), &[]);
        let span = frontend::std_range(&structure.span);
        let hir_annotations = lower_attributes(&structure.annotations, self.source_id, span)?;
        self.lower_class_like(
            Identifier::new(&structure.name.name),
            namespace.parts(),
            &structure.annotations,
            hir_annotations,
            &structure.generics,
            &structure.parents,
            &structure.body.fields,
            &structure.body.methods,
            true,
        )
    }

    fn lower_class_like(
        &self,
        name: Identifier,
        namespace: &[Identifier],
        annotations: &[Attribute],
        hir_annotations: Vec<HirAttribute>,
        generics: &[GenericParam],
        parents: &[Parent],
        fields: &[FieldDeclaration],
        methods: &[MethodDeclaration],
        is_value_type: bool,
    ) -> Result<HirStruct, HirError> {
        // 声明体方法就是该聚合 `Nominal` 上的固有方法；没有接收者参数的成员按静态成员处理，
        // MIR 侧仍发射普通 `Invoke`，不需要新的调用形态。
        // 泛型 owner 的声明体方法暂不支持：声明期只为单态 owner 铸造 callable 实例，
        // 且单态化入口（`CallableDeclaration`）只收 trait default / impl 成员 / 自由函数，
        // 泛型 owner 的成员没有可闭合的实例来源。这里保持结构化拒绝，不做静默降级——
        // 否则调用点会在 MIR 跌成不可达的 `UnresolvedCallableIdentity`，且没有任何前端诊断。
        if !generics.is_empty() {
            if let Some(method) = methods.first() {
                return Err(HirError::ClassBodyMethodDeclared {
                    aggregate: name.as_str().to_string(),
                    method: operator_member_name(&method.name, &method.annotations).as_str().to_string(),
                    source: nyar_types::DiagnosticSource { id: "<ast>".into(), revision: None },
                    span: nyar_types::DiagnosticSpan { start: method.name.span.start, end: method.name.span.end },
                });
            }
        }
        let methods = if generics.is_empty() {
            methods.iter().map(|method| self.lower_method(method)).collect::<Result<Vec<_>, _>>()?
        }
        else {
            Vec::new()
        };
        Ok(HirStruct {
            constructor_declaration: None,
            constructor_instance: None,
            name,
            namespace: namespace.to_vec(),
            doc: lower_documentation(annotations),
            generics: lower_generic_parameters(generics),
            parents: parents.iter().map(lower_parent).collect(),
            fields: fields.iter().map(lower_field).collect::<Result<Vec<_>, _>>()?,
            methods,
            visibility: lower_visibility(annotations),
            is_value_type,
            is_abstract: has_modifier(annotations, "abstract"),
            is_sealed: has_modifier(annotations, "sealed"),
            is_final: has_modifier(annotations, "final"),
            is_open: has_modifier(annotations, "open"),
            abstract_methods: Vec::new(),
            derives: lower_derives(annotations),
            // `[primitive("…")]` 是自举身份的唯一来源：下降期从声明注解读出，
            // 绑定期再冻结到引用该声明的 `ValkyrieType::Nominal` 上。
            primitive_identity: crate::compiler::semantic::closure::primitive_annotation_identity(annotations).map(Box::from),
            annotations: hir_annotations,
        })
    }

    fn lower_widget(&self, widget: &WidgetDeclaration) -> Result<HirWidget, HirError> {
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        for item in &widget.items {
            match item {
                StatementNode::Micro(micro) => methods.push(self.lower_micro(micro, &NamePath::default())?),
                _ => {}
            }
        }
        let lifecycle = HirWidgetLifecycle {
            has_on_mount: methods.iter().any(|m| m.name.as_str() == "on_mount"),
            has_on_unmount: methods.iter().any(|m| m.name.as_str() == "on_unmount"),
            has_on_update: methods.iter().any(|m| m.name.as_str() == "on_update"),
            has_before_update: methods.iter().any(|m| m.name.as_str() == "before_update"),
            has_after_update: methods.iter().any(|m| m.name.as_str() == "after_update"),
        };
        Ok(HirWidget {
            name: oak_identifier(&widget.name),
            doc: lower_documentation(&widget.annotations),
            generics: lower_generic_parameters(&widget.generics),
            fields,
            methods,
            visibility: lower_visibility(&widget.annotations),
            state_fields: Vec::new(),
            initial_state: Vec::new(),
            lifecycle,
        })
    }

    fn lower_singleton(&self, singleton: &SingletonDeclaration, namespace: &NamePath) -> Result<HirSingleton, HirError> {
        let _generic_scope = GenericBinderScope::enter(&generic_parameter_names(&singleton.generics), &[]);
        let all_methods: Vec<HirFunction> = singleton
            .body
            .methods
            .iter()
            .map(|method| self.lower_method(method))
            .collect::<Result<Vec<_>, _>>()?;
        let mut constructor: Option<Box<HirFunction>> = None;
        let mut finalizer: Option<Box<HirFunction>> = None;
        let mut ordinary_methods: Vec<HirFunction> = Vec::with_capacity(all_methods.len());
        for method in all_methods {
            match method.name.as_str() {
                SINGLETON_CONSTRUCTOR_NAME if constructor.is_none() => {
                    constructor = Some(Box::new(method));
                }
                SINGLETON_FINALIZER_NAME if finalizer.is_none() => {
                    finalizer = Some(Box::new(method));
                }
                _ => {
                    ordinary_methods.push(method);
                }
            }
        }
        Ok(HirSingleton {
            name: Identifier::new(&singleton.name.name),
            namespace: namespace.parts().to_vec(),
            doc: lower_documentation(&singleton.annotations),
            generics: lower_generic_parameters(&singleton.generics),
            parents: singleton.parents.iter().map(lower_parent).collect(),
            fields: singleton.body.fields.iter().map(lower_field).collect::<Result<Vec<_>, _>>()?,
            methods: ordinary_methods,
            visibility: lower_visibility(&singleton.annotations),
            derives: lower_derives(&singleton.annotations),
            is_lazy: has_modifier(&singleton.annotations, "lazy"),
            instance_name: Identifier::new(crate::compiler::mir::SINGLETON_INSTANCE_FIELD),
            constructor,
            finalizer,
            accessor_instance: None,
        })
    }

    fn lower_flags(&self, flags_decl: &Flags) -> Result<HirFlags, HirError> {
        Ok(HirFlags {
            name: oak_identifier(&flags_decl.name),
            doc: lower_documentation(&flags_decl.annotations),
            members: flags_decl.variants.iter().map(|member| self.lower_flag_member(member)).collect::<Result<Vec<_>, _>>()?,
            visibility: lower_visibility(&flags_decl.annotations),
        })
    }

    fn lower_flag_member(&self, member: &EnumVariant) -> Result<HirFlagMember, HirError> {
        let span = frontend::std_range(&member.span);
        Ok(HirFlagMember {
            name: oak_identifier(&member.name),
            doc: lower_documentation(&member.annotations),
            value: member
                .value
                .as_ref()
                .map(|expr| lower_term_expression(expr, self.source_id, span.clone()))
                .transpose()?
                .unwrap_or_else(|| HirExpr { kind: HirExprKind::Literal(HirLiteral::Integer64(0)), span: with_source(&span, self.source_id) }),
            is_combined: false,
        })
    }

    fn lower_trait(&self, trait_decl: &Trait) -> Result<HirTrait, HirError> {
        let associated_names =
            trait_decl.body.associated_types.iter().map(|item| oak_identifier(&item.name)).collect::<std::collections::BTreeSet<_>>();
        let generic_names = generic_parameter_names(&trait_decl.generics);
        let associated = associated_names.iter().map(ToString::to_string).collect::<Vec<_>>();
        let _generic_scope = GenericBinderScope::enter(&generic_names, &associated);
        let mut methods = Vec::new();
        let mut default_methods = Vec::new();
        for method in &trait_decl.body.methods {
            let lowered =
                crate::compiler::hir::lowering::generic_binders::lower_trait_method(self.lower_method(method)?, &associated_names);
            if lowered.is_abstract {
                methods.push(lowered);
            }
            else {
                default_methods.push(lowered);
            }
        }

        Ok(HirTrait {
            declaration: None,
            declaring_namespace: Default::default(),
            name: oak_identifier(&trait_decl.name),
            doc: lower_documentation(&trait_decl.annotations),
            generics: lower_generic_parameters(&trait_decl.generics),
            methods,
            associated_types: trait_decl
                .body
                .associated_types
                .iter()
                .map(|item| lower_trait_associated_type(item, self.source_id))
                .collect::<Result<Vec<_>, _>>()?,
            associated_constants: Vec::new(),
            super_traits: trait_decl
                .super_traits
                .iter()
                .map(crate::compiler::hir::lower_type_expression)
                .collect::<Result<Vec<_>, _>>()?,
            default_methods,
            visibility: lower_visibility(&trait_decl.annotations),
        })
    }

    fn lower_enums(&self, enum_decl: &Enums) -> Result<HirEnum, HirError> {
        let _generic_scope = GenericBinderScope::enter(&generic_parameter_names(&enum_decl.generics), &[]);
        let mut enum_def = match enum_decl.kind {
            EnumsKind::Unity => HirEnum::new_unity(oak_identifier(&enum_decl.name)),
            EnumsKind::Enums | EnumsKind::Enum => HirEnum::new(oak_identifier(&enum_decl.name)),
        };
        enum_def.doc = lower_documentation(&enum_decl.annotations);
        enum_def.visibility = lower_visibility(&enum_decl.annotations);
        enum_def.generics = lower_generic_parameters(&enum_decl.generics);
        enum_def.variants =
            enum_decl.variants.iter().map(|variant| self.lower_enum_variant(variant, enum_decl.kind)).collect::<Result<Vec<_>, _>>()?;
        enum_def.is_unity = enum_decl.kind == EnumsKind::Unity;
        Ok(enum_def)
    }

    fn lower_variant_decl(&self, variant_decl: &Variant) -> Result<HirEnum, HirError> {
        let _generic_scope = GenericBinderScope::enter(&generic_parameter_names(&variant_decl.generics), &[]);
        let mut enum_def = HirEnum::new_unity(oak_identifier(&variant_decl.name));
        enum_def.doc = lower_documentation(&variant_decl.annotations);
        enum_def.visibility = lower_visibility(&variant_decl.annotations);
        enum_def.generics = lower_generic_parameters(&variant_decl.generics);
        enum_def.variants = variant_decl
            .cases
            .iter()
            .enumerate()
            .map(|(index, case)| -> Result<_, HirError> {
                let span = frontend::std_range(&case.span);
                Ok(HirVariant {
                    declaration: None,
                    instance: None,
                    name: Identifier::new(&format!("case_{index}")),
                    doc: HirDocumentation::default(),
                    fields: Vec::new(),
                    result_type: None,
                    discriminator: Some(lower_term_expression(&case.body, self.source_id, span)?),
                    is_auto_constructor: false,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        enum_def.is_unity = true;
        Ok(enum_def)
    }

    fn lower_imply(&self, imply: &ImplyDeclaration, declaring_namespace: &NamePath) -> Result<HirImpl, HirError> {
        let generic_names = imply_generic_parameter_names(imply);
        let _generic_scope = GenericBinderScope::enter(&generic_names, &[]);
        let target = crate::compiler::hir::lower_type_expression(&imply.target_type)?;
        let mut associated_type_impls = Vec::new();
        let mut seen_associated = std::collections::BTreeSet::new();
        for item in &imply.associated_types {
            let name = oak_identifier(&item.name);
            let span = frontend::std_range(&item.span);
            // 同一 impl 重复声明同一关联类型无法确定唯一等式：拒绝失败闭合，禁止后者覆盖前者。
            if !seen_associated.insert(name.clone()) {
                return Err(frontend::ParseError::Invalid {
                    message: format!("impl 重复声明关联类型：{name}"),
                    span: Some(span),
                }
                .into());
            }
            let Some(equation) = item.default.as_ref()
            else {
                // `type Item`（无 `= T`）在 impl 里没有等式，是结构损坏而不是可推断的缺省。
                return Err(frontend::ParseError::Invalid {
                    message: format!("impl 关联类型缺少等式：{name}"),
                    span: Some(span),
                }
                .into());
            };
            let concrete_type = crate::compiler::hir::lower_type_expression(equation)?;
            let concrete_type = crate::compiler::hir::overload::substitution::substitute_type_self(&concrete_type, &target);
            associated_type_impls.push(HirAssociatedTypeImpl::new(name, concrete_type, with_source(&span, self.source_id)));
        }
        let trait_arguments = if let Some(ty) = imply.trait_type.as_ref() {
            if let TypeExpression::Apply(node) = ty {
                node.arguments.iter().map(crate::compiler::hir::lower_type_expression).collect::<Result<Vec<_>, _>>()?
            }
            else {
                Vec::new()
            }
        }
        else {
            Vec::new()
        };
        let mut methods = Vec::with_capacity(imply.methods.len());
        for method in &imply.methods {
            let mut lowered = self.lower_method(method)?;
            // impl 体内 `Self` 就是 impl 头部 target（含泛型实参）：在调用解析前把
            // 方法体声明槽位里的 `Self` 代入 target，与显式写 target 类型同型。
            crate::compiler::hir::lowering::substitute_impl_self_declarations(&mut lowered.body, &target);
            methods.push(lowered);
        }
        Ok(HirImpl {
            trait_declaration: None,
            declaring_namespace: declaring_namespace.clone(),
            trait_arguments,
            generics: lower_generic_parameters(&imply.generics),
            where_constraints: Vec::new(),
            target,
            trait_path: imply.trait_type.as_ref().map(lower_trait_path_from_type),
            methods,
            associated_type_impls,
            associated_const_impls: Vec::new(),
        })
    }

    fn lower_method(&self, method: &MethodDeclaration) -> Result<HirFunction, HirError> {
        let _generic_scope = GenericBinderScope::enter(&generic_parameter_names(&method.generics), &[]);
        let span = frontend::std_range(&method.span);
        Ok(HirFunction {
            declaration: None,
            instance: None,
            name: operator_member_name(&method.name, &method.annotations),
            declaring_namespace: NamePath::default(),
            doc: lower_documentation(&method.annotations),
            annotations: method
                .annotations
                .iter()
                .map(|attribute| lower_attribute(attribute, self.source_id, span.clone()))
                .collect::<Result<Vec<_>, _>>()?,
            generics: lower_generic_parameters(&method.generics),
            where_constraints: lower_where_constraints(&method.where_constraints, self.source_id)?,
            params: method.parameters.iter().map(|param| lower_param(param, self.source_id, span.clone())).collect::<Result<Vec<_>, _>>()?,
            return_type: method
                .return_type
                .as_ref()
                .map(crate::compiler::hir::lower_type_expression)
                .transpose()?
                .unwrap_or(ValkyrieType::Unit),
            body: method.body.as_ref().map(|block| lower_block(block, self.source_id, span.clone())).transpose()?.unwrap_or_else(|| HirBlock {
                statements: Vec::new(),
                expr: None,
                span: with_source(&span, self.source_id),
            }),
            span: with_source(&span, self.source_id),
            visibility: lower_visibility(&method.annotations),
            is_abstract: method.body.is_none() || has_modifier(&method.annotations, "abstract"),
            is_final: has_modifier(&method.annotations, "final"),
            is_virtual: has_modifier(&method.annotations, "virtual"),
            is_override: has_modifier(&method.annotations, "override"),
        })
    }

    fn lower_enum_variant(&self, variant: &EnumVariant, kind: EnumsKind) -> Result<HirVariant, HirError> {
        let span = frontend::std_range(&variant.span);
        let discriminator =
            variant.value.as_ref().map(|value| lower_term_expression(value, self.source_id, span.clone())).transpose()?.or_else(|| {
                if kind == EnumsKind::Unity {
                    crate::compiler::hir::lowering::attribute_lowering::tag_attribute_discriminator(
                        &variant.annotations,
                        self.source_id,
                        span.clone(),
                    )
                }
                else {
                    None
                }
            });
        Ok(HirVariant {
            declaration: None,
            instance: None,
            name: oak_identifier(&variant.name),
            doc: lower_documentation(&variant.annotations),
            fields: variant.fields.iter().map(lower_field).collect::<Result<Vec<_>, _>>()?,
            result_type: None,
            discriminator,
            is_auto_constructor: auto_constructor_attribute_count(&variant.annotations) == 1,
        })
    }
}
