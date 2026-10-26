use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{
    combined_content_hash,
    compiler_input::{DeclarationIndexEntry, DeclarationKey, DeclarationKind, DeclarationLink, EntryPointKind, ReferencePath},
};

use crate::compiler::syntax::oak::{ast, parse_source};

use super::{ScopedReference, collect::collect_items};

pub(crate) struct IndexedFile {
    pub(crate) source_digest: String,
    pub(crate) declarations: Vec<DeclarationIndexEntry>,
    /// 变体名 → 拥有该变体的 sum/unite 声明。
    pub(crate) variant_members: Vec<(String, DeclarationKey)>,
    /// 文件级语句（`using`、顶层 `let`/表达式）的自由名引用。
    pub(crate) module_references: BTreeSet<ScopedReference>,
    /// 文件级语句的成员选择名；与声明级选择名一样只参与成员提升。
    pub(crate) module_selectors: BTreeSet<String>,
}

pub(crate) fn index_source_file(dependency_key: &str, path: &str, source: &str, arch: &str) -> Result<IndexedFile, String> {
    let mut root = parse_source(source).map_err(|error| error.to_string())?;
    // TGrammar 模板分支必须按编译架构展开后再建索引：入口声明可能只在某个分支里，
    // 直接索引模板节点会让 `[export]` / `[main]` 在索引阶段不可见。
    crate::compiler::hir::lowering::template_expand::expand_templates_in_root(&mut root, source, arch).map_err(|error| error.to_string())?;
    let mut builder = FileBuilder::new(dependency_key, path);
    collect_items(&mut builder, &[], &root.items);
    Ok(IndexedFile {
        source_digest: combined_content_hash(&[source]),
        declarations: builder.declarations,
        variant_members: builder.variant_members,
        module_references: builder.module_references,
        module_selectors: builder.module_selectors,
    })
}

/// 单个文件索引过程中收集到的声明与文件级引用。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FileBuilder {
    dependency_key: String,
    source_path: String,
    declarations: Vec<DeclarationIndexEntry>,
    pub(crate) variant_members: Vec<(String, DeclarationKey)>,
    module_references: BTreeSet<ScopedReference>,
    module_selectors: BTreeSet<String>,
    import_paths: BTreeSet<String>,
}

impl FileBuilder {
    fn new(dependency_key: &str, source_path: &str) -> Self {
        Self { dependency_key: dependency_key.to_string(), source_path: source_path.to_string(), ..Self::default() }
    }

    pub(crate) fn register(
        &mut self,
        namespace: &[String],
        kind: DeclarationKind,
        name: &str,
        owner: Option<&str>,
        link: Option<DeclarationLink>,
        entry_point: Option<EntryPointKind>,
        references: References,
    ) -> DeclarationKey {
        let key = DeclarationKey {
            dependency_key: self.dependency_key.clone(),
            source_path: self.source_path.clone(),
            namespace: namespace.to_vec(),
            kind,
            owner: owner.map(str::to_string),
            name: name.to_string(),
        };
        self.declarations.push(DeclarationIndexEntry {
            key: key.clone(),
            link,
            entry_point,
            references: references.paths,
            selectors: references.selectors,
            import_paths: references.imports,
        });
        key
    }

    pub(crate) fn register_module_references(&mut self, namespace: &[String], references: References) {
        for path in references.paths {
            self.module_references.insert((namespace.to_vec(), path));
        }
        self.module_selectors.extend(references.selectors);
        self.import_paths.extend(references.imports);
    }
}

/// 单个声明（或文件级语句）引用的自由名路径、成员选择名与 import 路径。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct References {
    pub(crate) paths: BTreeSet<ReferencePath>,
    /// 成员选择名；只能与拥有聚合身份结合参与成员提升，不参与自由名解析。
    pub(crate) selectors: BTreeSet<String>,
    pub(crate) imports: BTreeSet<String>,
}

pub(crate) fn name_path_segments(path: &ast::NamePath) -> Vec<String> {
    path.parts.iter().map(|part| part.name.clone()).collect()
}
