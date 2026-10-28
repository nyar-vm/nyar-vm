//! 从 Oak AST 收集 host contract / provider inventory（禁止文本行扫描）。

use std::path::Path;

use crate::compiler::{
    semantic::{
        closure::{DeclarationKey, DeclarationKind, imply_owner_name, name_path_segments},
        operator_members,
    },
    syntax::oak::{
        ValkyrieRoot,
        ast::{Block, ImplyDeclaration, MethodDeclaration, MicroDeclaration, StatementNode},
        namespace_is_declaration_only, namespace_items,
    },
};

use super::{
    ast_attributes::{
        foreign_bindings_from_attributes, gate_attributes_from_items, has_host_contract_attribute, host_provider_contract_targets,
        when_attributes_from_items,
    },
    capability::required_capabilities_from_foreign_bindings,
    host::HostInventory,
    types::{PlannedHostContract, PlannedHostProvider},
};

pub(crate) fn collect_host_inventory_from_root(
    dependency_key: &str,
    source_path: &str,
    source_file: &Path,
    source: &str,
    root: &ValkyrieRoot,
    inventory: &mut HostInventory,
) {
    collect_host_from_items(dependency_key, source_path, source_file, source, &[], &root.items, inventory);
}

fn collect_host_from_items(
    dependency_key: &str,
    source_path: &str,
    source_file: &Path,
    source: &str,
    namespace: &[String],
    items: &[StatementNode],
    inventory: &mut HostInventory,
) {
    let mut current_namespace = namespace.to_vec();
    for item in items {
        match item {
            StatementNode::Namespace(namespace_decl) => {
                let declared = name_path_segments(&namespace_decl.name);
                if namespace_is_declaration_only(namespace_decl) {
                    current_namespace = declared;
                }
                else {
                    collect_host_from_items(
                        dependency_key,
                        source_path,
                        source_file,
                        source,
                        &declared,
                        namespace_items(namespace_decl),
                        inventory,
                    );
                }
            }
            StatementNode::Class(class) => {
                collect_type_members(
                    dependency_key,
                    source_path,
                    source_file,
                    source,
                    &current_namespace,
                    &class.name.name,
                    DeclarationKind::Class,
                    &class.body.methods,
                    inventory,
                );
            }
            StatementNode::Structure(structure) => {
                collect_type_members(
                    dependency_key,
                    source_path,
                    source_file,
                    source,
                    &current_namespace,
                    &structure.name.name,
                    DeclarationKind::Structure,
                    &structure.body.methods,
                    inventory,
                );
            }
            StatementNode::Trait(trait_decl) => {
                collect_type_members(
                    dependency_key,
                    source_path,
                    source_file,
                    source,
                    &current_namespace,
                    &trait_decl.name.name,
                    DeclarationKind::Method,
                    &trait_decl.body.methods,
                    inventory,
                );
            }
            StatementNode::Imply(imply) => {
                collect_imply_hosts(dependency_key, source_path, source_file, source, &current_namespace, imply, inventory)
            }
            StatementNode::Micro(micro) => {
                register_micro_hosts(
                    dependency_key,
                    source_path,
                    source_file,
                    source,
                    &current_namespace,
                    None,
                    DeclarationKind::Micro,
                    micro,
                    inventory,
                );
            }
            StatementNode::Statement(inner) => {
                collect_host_from_items(
                    dependency_key,
                    source_path,
                    source_file,
                    source,
                    &current_namespace,
                    std::slice::from_ref(inner.as_ref()),
                    inventory,
                );
            }
            _ => {}
        }
    }
}

fn collect_type_members(
    dependency_key: &str,
    source_path: &str,
    source_file: &Path,
    source: &str,
    namespace: &[String],
    owner: &str,
    kind: DeclarationKind,
    methods: &[MethodDeclaration],
    inventory: &mut HostInventory,
) {
    for method in methods {
        register_method_hosts(dependency_key, source_path, source_file, source, namespace, Some(owner), kind, method, inventory);
    }
}

fn collect_imply_hosts(
    dependency_key: &str,
    source_path: &str,
    source_file: &Path,
    source: &str,
    namespace: &[String],
    imply: &ImplyDeclaration,
    inventory: &mut HostInventory,
) {
    let owner = imply_owner_name(imply);
    for method in &imply.methods {
        register_method_hosts(
            dependency_key,
            source_path,
            source_file,
            source,
            namespace,
            owner.as_deref(),
            DeclarationKind::ImplyMethod,
            method,
            inventory,
        );
    }
}

fn register_method_hosts(
    dependency_key: &str,
    source_path: &str,
    source_file: &Path,
    source: &str,
    namespace: &[String],
    owner: Option<&str>,
    owner_kind: DeclarationKind,
    method: &MethodDeclaration,
    inventory: &mut HostInventory,
) {
    let annotations = &method.annotations;
    if !has_host_contract_attribute(annotations) && host_provider_contract_targets(annotations, source).is_empty() {
        return;
    }

    let name = operator_members::declared_member_name(&method.name.name, annotations);
    let kind = if owner_kind == DeclarationKind::ImplyMethod { DeclarationKind::ImplyMethod } else { DeclarationKind::Method };
    let key = declaration_key(dependency_key, source_path, namespace, owner, kind, &name);
    let qualified = qualified_declaration_name(namespace, owner, &name);
    let line = line_number(source, &method.span);

    if has_host_contract_attribute(annotations) {
        inventory.contracts.push(PlannedHostContract {
            id: qualified.clone(),
            key: key.clone(),
            source_file: source_file.to_path_buf(),
            line,
            has_local_fallback_body: method_has_local_fallback(method),
        });
    }

    register_providers_for_declaration(
        dependency_key,
        source_path,
        source_file,
        source,
        namespace,
        owner,
        kind,
        &name,
        &qualified,
        &key,
        annotations,
        line,
        inventory,
    );
}

fn register_micro_hosts(
    dependency_key: &str,
    source_path: &str,
    source_file: &Path,
    source: &str,
    namespace: &[String],
    owner: Option<&str>,
    kind: DeclarationKind,
    micro: &MicroDeclaration,
    inventory: &mut HostInventory,
) {
    let annotations = &micro.annotations;
    if !has_host_contract_attribute(annotations) && host_provider_contract_targets(annotations, source).is_empty() {
        return;
    }

    let name = operator_members::declared_member_name(&micro.name.name, annotations);
    let key = declaration_key(dependency_key, source_path, namespace, owner, kind, &name);
    let qualified = qualified_declaration_name(namespace, owner, &name);
    let line = line_number(source, &micro.span);

    if has_host_contract_attribute(annotations) {
        inventory.contracts.push(PlannedHostContract {
            id: qualified.clone(),
            key: key.clone(),
            source_file: source_file.to_path_buf(),
            line,
            has_local_fallback_body: micro_has_local_fallback(micro),
        });
    }

    register_providers_for_declaration(
        dependency_key,
        source_path,
        source_file,
        source,
        namespace,
        owner,
        kind,
        &name,
        &qualified,
        &key,
        annotations,
        line,
        inventory,
    );
}

fn register_providers_for_declaration(
    dependency_key: &str,
    source_path: &str,
    source_file: &Path,
    source: &str,
    namespace: &[String],
    owner: Option<&str>,
    kind: DeclarationKind,
    name: &str,
    qualified_symbol: &str,
    key: &DeclarationKey,
    annotations: &[crate::compiler::syntax::oak::ast::Attribute],
    line: usize,
    inventory: &mut HostInventory,
) {
    let foreign_binding_nodes = foreign_bindings_from_attributes(annotations);
    let foreign_bindings: Vec<String> = foreign_binding_nodes.iter().map(|binding| binding.display()).collect();
    let when_attributes = when_attributes_from_items(annotations);
    let gate_attributes = gate_attributes_from_items(annotations);
    let required_capabilities = required_capabilities_from_foreign_bindings(&foreign_binding_nodes);

    for contract in host_provider_contract_targets(annotations, source) {
        inventory.providers.push(PlannedHostProvider {
            contract,
            contract_key: None,
            symbol: qualified_symbol.to_string(),
            key: key.clone(),
            source_file: source_file.to_path_buf(),
            line,
            required_capabilities: required_capabilities.clone(),
            foreign_bindings: foreign_bindings.clone(),
            when_attributes: when_attributes.clone(),
            gate_attributes: gate_attributes.clone(),
        });
    }
}

fn declaration_key(
    dependency_key: &str,
    source_path: &str,
    namespace: &[String],
    owner: Option<&str>,
    kind: DeclarationKind,
    name: &str,
) -> DeclarationKey {
    DeclarationKey {
        dependency_key: dependency_key.to_string(),
        source_path: source_path.to_string(),
        namespace: namespace.to_vec(),
        kind,
        owner: owner.map(str::to_string),
        name: name.to_string(),
    }
}

fn qualified_declaration_name(namespace: &[String], owner: Option<&str>, symbol_name: &str) -> String {
    namespace.iter().map(String::as_str).chain(owner).chain(std::iter::once(symbol_name)).collect::<Vec<_>>().join("::")
}

fn line_number(source: &str, span: &oak_core::Range<usize>) -> usize {
    source.get(..span.start).map(|prefix| prefix.bytes().filter(|byte| *byte == b'\n').count() + 1).unwrap_or(1)
}

fn method_has_local_fallback(method: &MethodDeclaration) -> bool {
    method.body.as_ref().is_some_and(block_has_executable_body)
}

fn micro_has_local_fallback(micro: &MicroDeclaration) -> bool {
    !micro.is_abstract && block_has_executable_body(&micro.body)
}

fn block_has_executable_body(block: &Block) -> bool {
    !block.statements.is_empty()
}
