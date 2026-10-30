//! HIR lowering facade: parses front-end output and produces canonical HIR
//! modules ready for call resolution and MIR lowering.

use std::path::Path;

use nyar_types::{DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    driver::validation::validate_semantic_module,
    emitter::backend_contract::interop::{annotations_declare_external_import, validate_interop_surface},
    hir::{
        GenericBinderScope, hoist_anonymous_classes, lower_type_expression,
        overload::{resolve_hir_calls, validate_extractor_patterns},
        render_type_expression, validate_ast_root,
    },
    mir::{SINGLETON_CONSTRUCTOR_NAME, SINGLETON_FINALIZER_NAME},
    semantic::{
        closure::{GroupSemanticClosure, retain_demanded_declarations, source_group_closure_with_host_bindings},
        operator_members,
    },
    syntax::oak::{self as frontend, ValkyrieRoot},
    types::{Identifier, NamePath, SourceID, SourceSpan, hir::*},
};

mod ast_to_hir;
mod attribute_lowering;
mod collection_init;
mod contract_validation;
mod diagnostics;
pub mod errors;
mod expr_lowering;
mod generic_binders;
mod generic_instantiation;
mod macro_expand;
mod match_pattern_recovery;
mod nominal_declarations;
mod source_context;
mod source_group;
mod template_const;
pub(crate) mod template_expand;
mod trait_binding;
mod vx;

pub use super::CaptureAnalyzer;
pub use errors::HirError;

// Re-export sub-module symbols at the lowering facade boundary.
pub(crate) use diagnostics::{
    CompileWarningScope, identity_duplicated, index_exhausted, pipeline_rejected, push_compile_warning, take_compile_warnings,
};
use diagnostics::{plan_source, plan_span};

pub use contract_validation::validate_expr_call_contracts;
pub(crate) use contract_validation::{
    validate_block_call_contracts, validate_function_call_contracts, validate_hir_contract, validate_resolved_call_contracts,
};

#[doc(hidden)]
pub use nominal_declarations::validate_enum_discriminators;
pub(crate) use nominal_declarations::{
    collect_flags_layouts, collect_sum_declarations, compute_nominal_declarations, resolve_sum_variant_tag, validate_sum_type_discriminators,
};

pub(crate) use source_group::{
    SelfOwnerPath, compiled_program_from_hir_module, exportable_dependency_functions, register_root_type_alias_targets,
    root_type_alias_declarations, visible_nominal_type_names, visible_type_aliases,
};
pub use source_group::{build_generated_group, build_source_input_with_generated, compose_source_input_groups, validate_source_groups};

pub(crate) use generic_binders::{
    array_owner_declaration, bind_declared_nominal_types, combined_impl_generic_context, finalize_trait_method_signature,
    freeze_enum_generic_binders, freeze_function_generic_binders, freeze_generic_binders, freeze_impl_generic_binders,
    freeze_module_generic_binders, freeze_struct_generic_binders, freeze_trait_generic_binders, lookup_owner_generics_by_declaration,
    lower_trait_associated_type_references, lower_trait_method, max_hir_callable_instance_index, max_hir_declaration_index,
    propagate_declared_generic_binders, propagate_generic_binders_in_block, propagate_generic_binders_in_expr,
    propagate_generic_binders_in_where_constraints, propagate_impl_owner_type_generics, register_field_declarations,
    register_function_declarations,
};

pub(crate) use generic_instantiation::instantiate_generic_callables;

pub(crate) use attribute_lowering::{
    PropertyMethodKind, default_module_name, generic_parameter_names, has_modifier, imply_generic_parameter_names, is_property_accessor,
    lower_attribute, lower_attribute_argument_expression, lower_bound_identifier, lower_derives, lower_documentation, lower_field,
    lower_generic_parameter, lower_generic_parameters, lower_method_params, lower_name_path, lower_param, lower_parent, lower_property_params,
    lower_property_type, lower_trait_associated_type, lower_trait_path_from_type, lower_using, lower_visibility, lower_where_constraints,
    oak_identifier, operator_member_name, property_accessor_kind, property_is_abstract, property_is_final, property_is_lazy,
    property_is_override, property_is_static, property_is_virtual, tag_attribute_discriminator, with_source,
};

pub use ast_to_hir::AstToHir;

pub use nyar_types::compiler_input::{CompilerSourceFile, CompilerSourceGroup, CompilerSourceInput, GeneratedSourceGroup};

#[doc(hidden)]
pub use expr_lowering::lower_term_expression;
use expr_lowering::{extract_name_path, lower_block};

use macro_expand::expand_macros_in_root;
#[doc(hidden)]
pub use template_const::{eval_const_condition, parse_const_loop_range, parse_const_range_text};
#[doc(hidden)]
pub use template_expand::DEFAULT_COMPILE_ARCH;
#[doc(hidden)]
pub use template_expand::expand_templates_in_root;
#[doc(hidden)]
pub use vx::enhance_vx_widgets;

// ---------------------------------------------------------------------------
// ValkyrieCompiler facade struct and impl — the single entry point consumers
// hold.  All heavy lifting is delegated to the sub-modules above.
// ---------------------------------------------------------------------------

/// Minimal compiler facade that lowers parser output into HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValkyrieCompiler {
    /// Source id attached to synthesized spans during lowering.
    pub source_id: SourceID,
}

impl Default for ValkyrieCompiler {
    fn default() -> Self {
        Self::new(SourceID::default())
    }
}

impl ValkyrieCompiler {
    /// Creates a compiler facade bound to a source id.
    pub fn new(source_id: SourceID) -> Self {
        Self { source_id }
    }

    /// Validates an already materialized HIR module before it crosses into
    /// Semantic MIR. Cache consumers must use this instead of trusting the
    /// schema version of serialized HIR as a semantic guarantee.
    pub fn validate_hir_semantic_contract(&self, hir: &HirModule) -> Result<(), HirError> {
        validate_interop_surface(hir).map_err(HirError::BackendContractRejected)?;
        validate_hir_contract(hir)
    }

    /// Parses source text and lowers it into a minimal HIR module.
    pub fn compile_source(&self, source: &str) -> Result<HirModule, HirError> {
        self.compile_source_with_semantic_exports(source, &[])
    }

    /// Parses source text with nominal metadata exported by resolved
    /// dependencies. This metadata is supplied by the workspace resolver,
    /// never reconstructed from dependency source text in this consumer.
    pub fn compile_source_with_semantic_exports(
        &self,
        source: &str,
        imported_semantic_exports: &[HirDependencySemanticExport],
    ) -> Result<HirModule, HirError> {
        self.compile_source_with_semantic_exports_and_name(source, imported_semantic_exports, None)
    }

    /// 在解析调用前固定 Resolver 分配的模块身份。
    pub fn compile_source_with_semantic_exports_and_name(
        &self,
        source: &str,
        imported_semantic_exports: &[HirDependencySemanticExport],
        module_name: Option<NamePath>,
    ) -> Result<HirModule, HirError> {
        let mut root = frontend::parse_source(source)?;
        expand_macros_in_root(&mut root);
        expand_templates_in_root(&mut root, source, DEFAULT_COMPILE_ARCH)?;
        let hir = source_context::with_hir_lower_source(source, || {
            self.lower_root_with_semantic_exports_and_name(&root, imported_semantic_exports, module_name)
        })?;
        self.validate_hir_semantic_contract(&hir)?;
        Ok(hir)
    }

    /// 正式源码组的第一阶段：只物化 HIR，不解析调用。
    #[doc(hidden)]
    pub fn parse_source_group_without_call_resolution(
        &self,
        source: &str,
        imported_semantic_exports: &[HirDependencySemanticExport],
        module_name: Option<NamePath>,
        arch: &str,
    ) -> Result<HirModule, HirError> {
        let mut root = frontend::parse_source(source)?;
        expand_macros_in_root(&mut root);
        expand_templates_in_root(&mut root, source, arch)?;
        let hir = source_context::with_hir_lower_source(source, || {
            AstToHir::new(self.source_id).lower_root_without_call_resolution(&root, imported_semantic_exports, module_name)
        })?;
        validate_interop_surface(&hir).map_err(HirError::BackendContractRejected)?;
        Ok(hir)
    }

    /// Parses a source file and lowers it into a minimal HIR module.
    pub fn compile_path(&self, path: &Path) -> Result<HirModule, HirError> {
        let source = std::fs::read_to_string(path).map_err(|e| HirError::SourceReadFailed {
            path: path.display().to_string(),
            kind: e.kind(),
            source: plan_source(),
            span: plan_span(),
        })?;
        self.compile_source(&source)
    }

    /// Parses `.vx` source (Valkyrie + X-Grammar) and lowers into HIR with `view` → `render` normalization.
    pub fn compile_vx_source(&self, source: &str) -> Result<HirModule, HirError> {
        Err(HirError::VxUnsupported)
    }

    /// Parses a `.vx` file and lowers it into HIR with `view` → `render` normalization.
    pub fn compile_vx_path(&self, path: &Path) -> Result<HirModule, HirError> {
        let source = std::fs::read_to_string(path).map_err(|e| HirError::SourceReadFailed {
            path: path.display().to_string(),
            kind: e.kind(),
            source: plan_source(),
            span: plan_span(),
        })?;
        self.compile_vx_source(&source)
    }

    /// 集成测试夹具：将单一源码降低为成功载荷。
    #[doc(hidden)]
    pub fn compile_source_to_program(&self, source: &str) -> Result<nyar_types::CompiledProgram, HirError> {
        let hir_module = self.compile_source(source)?;
        compiled_program_from_hir_module(hir_module)
    }

    /// 从完整依赖顺序的源码快照构建一个语义闭包。
    #[doc(hidden)]
    pub fn compile_source_groups_to_program(&self, groups: &[CompilerSourceGroup]) -> Result<nyar_types::CompiledProgram, HirError> {
        self.compile_source_groups_to_program_with_host_bindings(groups, &[], DEFAULT_COMPILE_ARCH)
    }

    /// 从「应用组 + 独立生成组」的完整输入构建程序。
    #[doc(hidden)]
    pub fn compile_source_input_to_program_with_host_bindings(
        &self,
        input: &CompilerSourceInput,
        host_bindings: &[crate::compiler::emitter::pipeline::CompilerHostProviderBinding],
        arch: &str,
    ) -> Result<nyar_types::CompiledProgram, HirError> {
        let groups = compose_source_input_groups(input)?;
        self.compile_source_groups_to_program_with_host_bindings(&groups, host_bindings, arch)
    }

    /// 从完整依赖顺序的源码快照构建语义闭包，并应用 Resolver 选定的 host provider 绑定。
    #[doc(hidden)]
    pub fn compile_source_groups_to_program_with_host_bindings(
        &self,
        groups: &[CompilerSourceGroup],
        host_bindings: &[crate::compiler::emitter::pipeline::CompilerHostProviderBinding],
        arch: &str,
    ) -> Result<nyar_types::CompiledProgram, HirError> {
        let entry_indices = groups.iter().enumerate().filter(|(_, group)| group.is_entry).map(|(index, _)| index).collect::<Vec<_>>();
        let mut hir_groups = self.resolve_source_groups_with_host_bindings(groups, host_bindings, arch)?;
        let final_index = hir_groups
            .len()
            .checked_sub(1)
            .ok_or_else(|| HirError::EmptySemanticSourceGroupPlan { source: plan_source(), span: plan_span() })?;
        let final_hir = hir_groups.pop().ok_or_else(|| HirError::EmptySemanticSourceGroupPlan { source: plan_source(), span: plan_span() })?;
        let all_hirs = std::iter::once(&final_hir).chain(hir_groups.iter()).collect::<Vec<_>>();
        let host_runtime = crate::compiler::mir::ssa::contracts::allocate_program_host_runtime(&all_hirs);
        let dispatch = crate::compiler::mir::ssa::generic_call_instantiation::collect_dispatch_entries(&all_hirs);
        let mut mir_groups = hir_groups
            .iter()
            .map(|hir| crate::compiler::mir::MirLowerer::lower_module_semantic_with_host(hir, Some(&host_runtime), false, &dispatch))
            .collect::<Vec<_>>();
        let mut final_mir = crate::compiler::mir::MirLowerer::lower_module_semantic_with_host(&final_hir, Some(&host_runtime), true, &dispatch);
        for index in entry_indices {
            if index != final_index {
                crate::compiler::emitter::pipeline::merge_entry_group_surface(&mut final_mir, &mir_groups[index])?;
            }
        }
        let modules = std::iter::once(&final_mir).chain(mir_groups.iter()).collect::<Vec<_>>();
        let functions = modules.iter().flat_map(|module| module.functions.iter().cloned()).collect::<Vec<_>>();
        let imports = modules.iter().flat_map(|module| module.external_calls.iter().cloned()).collect::<Vec<_>>();
        let structures = modules.iter().flat_map(|module| module.structs.iter().cloned()).collect::<Vec<_>>();
        let type_identities = crate::compiler::mir::ssa::type_identity_table(&functions, &imports, &structures, &final_mir.sum_types);
        final_mir.type_identities = type_identities.clone();
        for module in &mut mir_groups {
            module.type_identities = type_identities.clone();
        }
        crate::compiler::emitter::pipeline::prune_unselected_host_provider_external_calls(
            &mut mir_groups,
            &mut final_mir,
            &all_hirs,
            host_bindings,
        )?;
        if !mir_groups.is_empty() {
            crate::compiler::emitter::pipeline::link_reachable_dependency_mir_with_hir(
                &mut final_mir,
                &mir_groups,
                &all_hirs,
                &std::collections::BTreeMap::new(),
            )?;
        }
        let resolved_host_bindings = crate::compiler::emitter::pipeline::resolve_host_provider_bindings(&all_hirs, host_bindings)?;
        let host_binding_remaps =
            crate::compiler::emitter::pipeline::apply_host_provider_bindings(&mut final_mir, &mir_groups, &resolved_host_bindings)?;
        // provider 依赖闭包：链接 BFS 在重映射前不可达 provider，其函数体引用的 wasm 导入
        // 与本地 helper 不会随定义迁入消费模块。绑定安装后由同一 BFS 再走一遍闭合闭包；
        // 合同调用已全部改写为 provider 身份，退役合同体不会被重新拉入。
        // 首轮链接迁入消费模块的依赖体副本已被同一改写表重写，二次链接必须携带该事实
        // 才能把依赖原件规范化到绑定后状态并核对冻结身份下的唯一副本。
        if !mir_groups.is_empty() && !resolved_host_bindings.is_empty() {
            crate::compiler::emitter::pipeline::link_reachable_dependency_mir_with_hir(
                &mut final_mir,
                &mir_groups,
                &all_hirs,
                &host_binding_remaps,
            )?;
        }
        crate::compiler::emitter::pipeline::compile_linked_semantic_mir(&final_mir).map_err(pipeline_rejected)
    }

    #[doc(hidden)]
    pub fn resolve_source_groups(&self, groups: &[CompilerSourceGroup], arch: &str) -> Result<Vec<HirModule>, HirError> {
        self.resolve_source_groups_with_host_bindings(groups, &[], arch)
    }

    /// 从完整依赖顺序的源码快照构建语义闭包，并把 Resolver 选定的 host provider 声明纳入 body lowering。
    pub fn resolve_source_groups_with_host_bindings(
        &self,
        groups: &[CompilerSourceGroup],
        host_bindings: &[crate::compiler::emitter::pipeline::CompilerHostProviderBinding],
        arch: &str,
    ) -> Result<Vec<HirModule>, HirError> {
        validate_source_groups(groups)?;
        let closure = source_group_closure_with_host_bindings(groups, host_bindings, arch)?;
        let mut exports = std::collections::BTreeMap::<String, HirDependencySemanticExport>::new();
        let mut hir_groups = Vec::with_capacity(groups.len());
        let mut next_declaration = 0u32;
        let mut next_instance = 0u32;
        let mut next_field = 0u32;
        for (group, selection) in groups.iter().zip(&closure.groups) {
            let dependency_exports = group
                .direct_dependencies
                .iter()
                .map(|name| {
                    exports.get(name).cloned().ok_or_else(|| HirError::DependencyExportUnavailable {
                        group: group.name.clone(),
                        dependency: name.clone(),
                        source: plan_source(),
                        span: plan_span(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut hir_module = self.parse_source_files_without_call_resolution(
                group,
                selection,
                &dependency_exports,
                Some(NamePath::new(vec![Identifier::new(&group.name)])),
                arch,
            )?;
            hir_module.dependency_key = group.dependency_key.clone();
            hir_module.source_path = group.sources.first().map(|file| file.path.clone()).unwrap_or_else(|| format!("{}.v", group.name));
            register_function_declarations(&mut hir_module, &mut next_declaration, &mut next_instance)?;
            register_field_declarations(&mut hir_module, &mut next_field)?;
            freeze_module_generic_binders(&mut hir_module);
            bind_declared_nominal_types(&mut hir_module)?;
            trait_binding::bind_trait_declarations(&mut hir_module)?;
            // 集合字面量转换跑在 trait 绑定之后、本组导出快照与调用解析之前：
            // 依赖组的合同事实已经随 `dependency_exports` 进入 `imported_semantic_exports`，
            // 而本组 export 尚未构建，转换只改体内表达式、不扰动声明面导出事实。
            collection_init::transform_collection_literals(&mut hir_module)?;
            let export = HirDependencySemanticExport {
                module: NamePath::new(vec![Identifier::new(&group.name)]),
                functions: exportable_dependency_functions(&hir_module.functions),
                structs: hir_module.structs.clone(),
                enums: hir_module.enums.clone(),
                traits: hir_module.traits.clone(),
                type_aliases: hir_module.type_aliases.clone(),
                impls: hir_module.impls.clone(),
            };
            if exports.insert(group.dependency_key.clone(), export).is_some() {
                return Err(HirError::DependencyExportIdentityCollision {
                    dependency_key: group.dependency_key.clone(),
                    source: plan_source(),
                    span: plan_span(),
                });
            }
            hir_groups.push(hir_module);
        }
        for hir in &mut hir_groups {
            resolve_hir_calls(hir);
        }
        // 单态化固定点必须在全部源码组的调用解析之后、语义合同校验与 MIR 之前运行。
        instantiate_generic_callables(&mut hir_groups);
        for hir in &hir_groups {
            validate_extractor_patterns(hir)?;
            self.validate_hir_semantic_contract(hir)?;
        }
        Ok(hir_groups)
    }

    fn parse_source_files_without_call_resolution(
        &self,
        group: &CompilerSourceGroup,
        selection: &GroupSemanticClosure,
        imported_semantic_exports: &[HirDependencySemanticExport],
        module_name: Option<NamePath>,
        arch: &str,
    ) -> Result<HirModule, HirError> {
        if selection.body_lowering_source_paths.is_empty() && selection.body_lowering_declarations.is_empty() {
            return AstToHir::new(self.source_id).lower_root_without_call_resolution(
                &ValkyrieRoot { items: Vec::new() },
                imported_semantic_exports,
                module_name,
            );
        }
        let mut prepared = Vec::new();
        for file in &group.sources {
            if !selection.body_lowering_source_paths.contains(&file.path) {
                continue;
            }
            let root = self.prepare_source_file_root(group, selection, file, arch)?;
            prepared.push((file, root));
        }
        if prepared.is_empty() {
            return Err(HirError::NoBodySourceInGroup {
                dependency_key: group.dependency_key.clone(),
                source: DiagnosticSource { id: group.dependency_key.clone(), revision: None },
                span: DiagnosticSpan { start: 0, end: 0 },
            }
            .into());
        }
        let roots = prepared.iter().map(|(_, root)| root).collect::<Vec<_>>();
        let _type_alias_scope = crate::compiler::hir::ModuleTypeAliasScope::enter(visible_type_aliases(&roots, imported_semantic_exports));
        let _nominal_scope = crate::compiler::hir::NominalTypeScope::enter(visible_nominal_type_names(&roots, imported_semantic_exports));
        register_root_type_alias_targets(&roots)?;
        let mut merged: Option<HirModule> = None;
        for (file, root) in &prepared {
            let module = self.lower_prepared_source_file(file, root, imported_semantic_exports, module_name.clone())?;
            if let Some(target) = &mut merged {
                target.imports.extend(module.imports);
                target.warnings.extend(module.warnings);
                target.submodules.extend(module.submodules);
                target.functions.extend(module.functions);
                target.structs.extend(module.structs);
                target.enums.extend(module.enums);
                target.imported_enums.extend(module.imported_enums);
                target.flags.extend(module.flags);
                target.traits.extend(module.traits);
                target.impls.extend(module.impls);
                target.type_functions.extend(module.type_functions);
                target.type_families.extend(module.type_families);
                target.widgets.extend(module.widgets);
                target.singletons.extend(module.singletons);
                target.statements.extend(module.statements);
                target.type_aliases.extend(module.type_aliases);
            }
            else {
                merged = Some(module);
            }
        }
        let hir = merged.expect("prepared source files imply a merged HirModule");
        validate_interop_surface(&hir).map_err(HirError::BackendContractRejected)?;
        Ok(hir)
    }

    /// 解析一个源文件并完成宏/模板展开；别名收集与 HIR lowering 都在展开结果上进行。
    fn prepare_source_file_root(
        &self,
        group: &CompilerSourceGroup,
        selection: &GroupSemanticClosure,
        file: &CompilerSourceFile,
        arch: &str,
    ) -> Result<ValkyrieRoot, HirError> {
        let mut root = self.prepare_nominal_inventory_root(group, file, arch)?;
        retain_demanded_declarations(&mut root.items, &group.dependency_key, &file.path, &selection.body_lowering_declarations);
        Ok(root)
    }

    /// 解析组内单个文件并展开宏/模板，供名义类型 inventory 与 body lowering 共用。
    fn prepare_nominal_inventory_root(
        &self,
        group: &CompilerSourceGroup,
        file: &CompilerSourceFile,
        arch: &str,
    ) -> Result<ValkyrieRoot, HirError> {
        let mut root = frontend::parse_source(&file.source).map_err(|parse_err| HirError::SourceFileRejected {
            dependency_key: group.dependency_key.clone(),
            path: file.path.clone(),
            error: parse_err,
        })?;
        expand_macros_in_root(&mut root);
        expand_templates_in_root(&mut root, &file.source, arch)?;
        Ok(root)
    }

    /// 降低一个已解析并展开的源文件；类型别名作用域由调用方按组建立。
    fn lower_prepared_source_file(
        &self,
        file: &CompilerSourceFile,
        root: &ValkyrieRoot,
        imported_semantic_exports: &[HirDependencySemanticExport],
        module_name: Option<NamePath>,
    ) -> Result<HirModule, HirError> {
        source_context::with_hir_lower_source(&file.source, || {
            AstToHir::new(self.source_id).lower_prepared_root(root, imported_semantic_exports, module_name)
        })
    }

    /// Lowers parser output into a HIR module.
    pub fn lower_root(&self, root: &ValkyrieRoot) -> Result<HirModule, HirError> {
        self.lower_root_with_semantic_exports(root, &[])
    }

    /// Lowers parser output with resolved nominal dependency exports in scope.
    pub fn lower_root_with_semantic_exports(
        &self,
        root: &ValkyrieRoot,
        imported_semantic_exports: &[HirDependencySemanticExport],
    ) -> Result<HirModule, HirError> {
        AstToHir::new(self.source_id).lower_root_with_semantic_exports(root, imported_semantic_exports)
    }

    /// 在 HIR 调用解析前覆盖模块身份。
    pub fn lower_root_with_semantic_exports_and_name(
        &self,
        root: &ValkyrieRoot,
        imported_semantic_exports: &[HirDependencySemanticExport],
        module_name: Option<NamePath>,
    ) -> Result<HirModule, HirError> {
        AstToHir::new(self.source_id).lower_root_with_semantic_exports_and_name(root, imported_semantic_exports, module_name)
    }
}
