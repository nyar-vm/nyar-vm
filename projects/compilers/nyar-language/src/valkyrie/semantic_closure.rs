//! 入口驱动的语义闭包规划：namespace 感知的声明图、body lowering 边界与内容摘要。
//!
//! 闭包由声明身份驱动，而不是文件短名近似：
//!
//! - 种子是入口组的声明；依赖组只在自身声明被需求时进入 body lowering；
//! - 自由名引用按「限定路径 → 当前 namespace → 父 namespace」解析；
//! - 成员名引用先按同一 namespace 链解析，再回退到全库成员名索引
//!   （调用解析是接收者类型驱动的，索引阶段没有类型，回退是显式的过近似）；
//! - 摘要绑定规划版本、依赖身份、被编译声明身份与这些声明的源文件内容。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use nyar_types::combined_content_hash;

use crate::valkyrie::frontend::{ast, parse_source};

/// 闭包规划版本；规划语义变化必须递增，旧摘要不得跨版本复用。
const CLOSURE_PLANNER_VERSION: &str = "semantic-closure-v2";

/// 预 HIR 声明种类；与后续 `ItemId` 分配一一对应，不依赖短名认亲。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclarationKind {
    /// 函数（`micro`）声明。
    Micro,
    /// 结构体声明。
    Structure,
    /// 类声明。
    Class,
    /// trait 声明。
    Trait,
    /// enum 声明。
    Enum,
    /// unite 声明。
    Union,
    /// flags 声明。
    Flags,
    /// 类型别名声明。
    TypeAlias,
    /// 类型函数声明。
    TypeFunction,
    /// 成员方法声明。
    Method,
    /// trait 实现方法声明。
    ImplyMethod,
}

impl DeclarationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Micro => "micro",
            Self::Structure => "structure",
            Self::Class => "class",
            Self::Trait => "trait",
            Self::Enum => "enum",
            Self::Union => "union",
            Self::Flags => "flags",
            Self::TypeAlias => "type_alias",
            Self::TypeFunction => "type_function",
            Self::Method => "method",
            Self::ImplyMethod => "imply_method",
        }
    }

    /// 是否携带需要 lowering 的函数体。
    pub fn is_body_bearing(self) -> bool {
        matches!(self, Self::Micro | Self::TypeFunction | Self::Method | Self::ImplyMethod)
    }
}

/// 名字引用路径（`std::collection::hash_utf8` → `["std","collection","hash_utf8"]`）。
pub type ReferencePath = Vec<String>;

/// namespace 段 + 文件名路径引用的组合（文件级语句所属 namespace 与它引用的名字）。
type ScopedReference = (Vec<String>, ReferencePath);

/// 预 HIR 声明身份：依赖组 + 源文件 + namespace + 种类 + 名字；成员方法另带拥有者聚合名。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclarationKey {
    /// Resolver 分配的依赖组身份。
    pub dependency_key: String,
    /// 仓相对源码路径。
    pub source_path: String,
    /// 声明所在 namespace 段；`namespace a::b;` → `["a","b"]`，无 namespace → 空。
    pub namespace: Vec<String>,
    /// 顶层声明种类。
    pub kind: DeclarationKind,
    /// 成员方法所属的结构/类/trait 名；自由声明为 `None`。
    pub owner: Option<String>,
    /// 声明短名。
    pub name: String,
}

impl DeclarationKey {
    /// 稳定编码身份；只用于审核与诊断，身份匹配必须使用结构字段。
    pub fn encoded(&self) -> String {
        format!(
            "{}::{}::{}::{}::{}::{}",
            self.dependency_key,
            self.source_path,
            self.kind.as_str(),
            self.namespace.join("::"),
            self.owner.as_deref().unwrap_or(""),
            self.name
        )
    }
}

/// 单条声明索引：不降低函数体，只记录该声明签名与函数体引用的名字路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationIndexEntry {
    /// 稳定声明身份。
    pub key: DeclarationKey,
    /// 该声明签名与函数体内出现的名字路径（未解析）。
    pub references: BTreeSet<ReferencePath>,
    /// 文件级 `using` 路径；当前解析器不消费 import，这里只保留为诊断与后续绑定输入。
    pub import_paths: BTreeSet<String>,
}

/// 单组语义闭包切片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSemanticClosure {
    /// 依赖组身份。
    pub dependency_key: String,
    /// 进入 body lowering 的仓相对路径。
    pub body_lowering_source_paths: BTreeSet<String>,
    /// 本组 body lowering 覆盖的声明身份。
    pub body_lowering_declarations: BTreeSet<DeclarationKey>,
}

/// 入口驱动的完整语义闭包：声明索引 + body lowering 边界 + 内容摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticClosure {
    /// 各依赖组的 body lowering 切片。
    pub groups: Vec<GroupSemanticClosure>,
    /// 全 inventory 声明索引（含不可达文件）。
    pub declaration_index: BTreeMap<DeclarationKey, DeclarationIndexEntry>,
    /// 入口闭包内被引用的声明身份。
    pub demanded_declarations: BTreeSet<DeclarationKey>,
    /// 未能建立声明索引的 inventory 文件（`依赖组::路径` → 失败原因）。
    pub unindexed_source_files: BTreeMap<String, String>,
    /// 闭包稳定摘要。
    pub digest: String,
}

/// 从入口组与其依赖 inventory 规划完整语义闭包。
pub fn plan_semantic_closure(groups: &[(&str, bool, &[(String, String)])]) -> SemanticClosure {
    let mut indexed_files = BTreeMap::<(String, String), IndexedFile>::new();
    let mut declaration_index = BTreeMap::<DeclarationKey, DeclarationIndexEntry>::new();
    let mut names = BTreeMap::<(Vec<String>, String), BTreeSet<DeclarationKey>>::new();
    let mut members = BTreeMap::<String, BTreeSet<DeclarationKey>>::new();
    let mut unindexed_source_files = BTreeMap::<String, String>::new();

    for (dependency_key, _, files) in groups.iter() {
        for (path, source) in files.iter() {
            match index_source_file(dependency_key, path, source) {
                Ok(indexed) => {
                    for entry in &indexed.declarations {
                        names.entry((entry.key.namespace.clone(), entry.key.name.clone())).or_default().insert(entry.key.clone());
                        if matches!(entry.key.kind, DeclarationKind::Method | DeclarationKind::ImplyMethod) {
                            members.entry(entry.key.name.clone()).or_default().insert(entry.key.clone());
                        }
                        declaration_index.insert(entry.key.clone(), entry.clone());
                    }
                    for (variant, owner) in &indexed.variant_members {
                        members.entry(variant.clone()).or_default().insert(owner.clone());
                    }
                    indexed_files.insert((dependency_key.to_string(), path.clone()), indexed);
                }
                Err(reason) => {
                    unindexed_source_files.insert(format!("{dependency_key}::{path}"), reason);
                }
            }
        }
    }

    let mut demanded = BTreeSet::new();
    let mut queue = VecDeque::new();
    let mut touched_files = BTreeSet::<(String, String)>::new();
    for (dependency_key, is_entry, files) in groups.iter() {
        if !*is_entry {
            continue;
        }
        // 入口文件整体是待编译程序：文件进入 body lowering，且其声明的引用参与闭包展开。
        for (path, _) in files.iter() {
            let file_id = (dependency_key.to_string(), path.clone());
            let Some(indexed) = indexed_files.get(&file_id)
            else {
                continue;
            };
            for entry in &indexed.declarations {
                queue.push_back(entry.key.clone());
            }
            // 文件级 `using`/顶层语句与声明外壳的引用必须与声明引用一起展开，
            // 否则只含模块级语句的入口文件、以及随文件物化的类型/impl 外壳都会缺依赖。
            if touched_files.insert(file_id.clone()) {
                push_scoped_references(&indexed.module_references, &names, &members, &mut queue);
                push_scoped_references(&indexed.shell_references, &names, &members, &mut queue);
            }
        }
    }
    while let Some(key) = queue.pop_front() {
        if !demanded.insert(key.clone()) {
            continue;
        }
        let entry = match declaration_index.get(&key) {
            Some(entry) => entry.clone(),
            None => continue,
        };
        if touched_files.insert((key.dependency_key.clone(), key.source_path.clone())) {
            if let Some(file) = indexed_files.get(&(key.dependency_key.clone(), key.source_path.clone())) {
                push_scoped_references(&file.module_references, &names, &members, &mut queue);
                push_scoped_references(&file.shell_references, &names, &members, &mut queue);
            }
        }
        // 成员方法被需求时，其拥有者聚合的声明外壳同样必须进入闭包。
        if let Some(owner) = &key.owner {
            for resolved in resolve_reference(&[owner.clone()], &key.namespace, &names, &members) {
                queue.push_back(resolved);
            }
        }
        for reference in &entry.references {
            for resolved in resolve_reference(reference, &key.namespace, &names, &members) {
                queue.push_back(resolved);
            }
        }
    }

    let mut closure_groups = Vec::with_capacity(groups.len());
    for (dependency_key, is_entry, files) in groups.iter() {
        let dependency_key = *dependency_key;
        let mut body_lowering_source_paths = BTreeSet::new();
        if *is_entry {
            for (path, _) in files.iter() {
                body_lowering_source_paths.insert(path.clone());
            }
        }
        let mut body_lowering_declarations = BTreeSet::new();
        for key in &demanded {
            if key.dependency_key == dependency_key {
                body_lowering_declarations.insert(key.clone());
                body_lowering_source_paths.insert(key.source_path.clone());
            }
        }
        closure_groups.push(GroupSemanticClosure {
            dependency_key: dependency_key.to_string(),
            body_lowering_source_paths,
            body_lowering_declarations,
        });
    }

    let digest = compute_closure_digest(&closure_groups, &indexed_files);
    SemanticClosure { groups: closure_groups, declaration_index, demanded_declarations: demanded, unindexed_source_files, digest }
}

/// 解析一组「namespace 段 + 名字路径」引用并把命中的声明身份入队。
fn push_scoped_references(
    references: &BTreeSet<ScopedReference>,
    names: &BTreeMap<(Vec<String>, String), BTreeSet<DeclarationKey>>,
    members: &BTreeMap<String, BTreeSet<DeclarationKey>>,
    queue: &mut VecDeque<DeclarationKey>,
) {
    for (namespace, reference) in references {
        for resolved in resolve_reference(reference, namespace, names, members) {
            queue.push_back(resolved);
        }
    }
}

/// 解析单个名字路径引用，返回命中的声明身份集合。
///
/// 命中规则与 `path-resolution.md` 一致：限定路径先按绝对路径，再按当前 namespace 逐级前缀；
/// 单段名字先沿 namespace 链向外查找，链上没有命中时回退到成员名索引（显式过近似）。
fn resolve_reference(
    reference: &[String],
    namespace: &[String],
    names: &BTreeMap<(Vec<String>, String), BTreeSet<DeclarationKey>>,
    members: &BTreeMap<String, BTreeSet<DeclarationKey>>,
) -> BTreeSet<DeclarationKey> {
    let name = match reference.last() {
        Some(name) => name,
        None => return BTreeSet::new(),
    };
    if reference.len() > 1 {
        let absolute = reference[..reference.len() - 1].to_vec();
        if let Some(keys) = names.get(&(absolute, name.clone())) {
            return keys.clone();
        }
        for depth in (0..=namespace.len()).rev() {
            let mut scoped = namespace[..depth].to_vec();
            scoped.extend_from_slice(&reference[..reference.len() - 1]);
            if let Some(keys) = names.get(&(scoped, name.clone())) {
                return keys.clone();
            }
        }
        return BTreeSet::new();
    }
    if !is_builtin_symbol(name) {
        for depth in (0..=namespace.len()).rev() {
            let scoped = namespace[..depth].to_vec();
            if let Some(keys) = names.get(&(scoped, name.clone())) {
                return keys.clone();
            }
        }
    }
    members.get(name).cloned().unwrap_or_default()
}

fn compute_closure_digest(groups: &[GroupSemanticClosure], files: &BTreeMap<(String, String), IndexedFile>) -> String {
    let mut parts = vec![format!("planner:{CLOSURE_PLANNER_VERSION}")];
    for group in groups {
        parts.push(format!("group:{}", group.dependency_key));
        for key in &group.body_lowering_declarations {
            parts.push(format!("decl:{}", key.encoded()));
        }
        for path in &group.body_lowering_source_paths {
            parts.push(format!("path:{path}"));
            if let Some(file) = files.get(&(group.dependency_key.clone(), path.clone())) {
                parts.push(format!("content:{}", file.source_digest));
            }
        }
    }
    let borrowed = parts.iter().map(String::as_str).collect::<Vec<_>>();
    combined_content_hash(&borrowed)
}

struct IndexedFile {
    source_digest: String,
    declarations: Vec<DeclarationIndexEntry>,
    /// 变体名 → 拥有该变体的 sum/unite 声明。
    variant_members: Vec<(String, DeclarationKey)>,
    /// 文件级语句（`using`、顶层 `let`/表达式）的引用。
    module_references: BTreeSet<ScopedReference>,
    /// 本文件所有声明外壳（类型声明、聚合、trait、别名、impl 块签名）的引用。
    ///
    /// HIR 会物化进入闭包的每个文件的声明外壳，因此外壳引用必须随文件一起进入闭包，
    /// 否则外壳会出现「声明在闭包里、它引用的类型/trait 不在闭包里」的假失败。
    shell_references: BTreeSet<ScopedReference>,
}

fn index_source_file(dependency_key: &str, path: &str, source: &str) -> Result<IndexedFile, String> {
    let root = parse_source(source).map_err(|error| error.to_string())?;
    let mut builder = FileBuilder::new(dependency_key, path);
    collect_items(&mut builder, &[], &root.items);
    Ok(IndexedFile {
        source_digest: combined_content_hash(&[source]),
        declarations: builder.declarations,
        variant_members: builder.variant_members,
        module_references: builder.module_references,
        shell_references: builder.shell_references,
    })
}

/// 单个文件索引过程中收集到的声明与文件级引用。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FileBuilder {
    dependency_key: String,
    source_path: String,
    declarations: Vec<DeclarationIndexEntry>,
    variant_members: Vec<(String, DeclarationKey)>,
    module_references: BTreeSet<ScopedReference>,
    /// 本文件所有声明外壳（类型声明、聚合、trait、别名、impl 块签名）的引用。
    shell_references: BTreeSet<ScopedReference>,
    import_paths: BTreeSet<String>,
}

impl FileBuilder {
    fn new(dependency_key: &str, source_path: &str) -> Self {
        Self { dependency_key: dependency_key.to_string(), source_path: source_path.to_string(), ..Self::default() }
    }

    fn register(
        &mut self,
        namespace: &[String],
        kind: DeclarationKind,
        name: &str,
        owner: Option<&str>,
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
        if !kind.is_body_bearing() {
            self.register_shell_references(namespace, &references);
        }
        self.declarations.push(DeclarationIndexEntry { key: key.clone(), references: references.paths, import_paths: references.imports });
        key
    }

    /// 登记始终物化的声明外壳引用：HIR 会物化进入闭包的每个文件的类型声明与 impl 块签名，
    /// 这些外壳引用的类型/trait 必须随文件一起进入闭包，否则会出现「声明在闭包内、其依赖不在」的假失败。
    fn register_shell_references(&mut self, namespace: &[String], references: &References) {
        for path in &references.paths {
            self.shell_references.insert((namespace.to_vec(), path.clone()));
        }
    }

    fn register_module_references(&mut self, namespace: &[String], references: References) {
        for path in references.paths {
            self.module_references.insert((namespace.to_vec(), path));
        }
        self.import_paths.extend(references.imports);
    }
}

/// 单个声明（或文件级语句）引用的名字路径与 import 路径。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct References {
    paths: BTreeSet<ReferencePath>,
    imports: BTreeSet<String>,
}

fn name_path_segments(path: &ast::NamePath) -> Vec<String> {
    path.parts.iter().map(|part| part.name.clone()).collect()
}

fn collect_items(builder: &mut FileBuilder, current_namespace: &[String], items: &[ast::StatementNode]) {
    let mut namespace = current_namespace.to_vec();
    for item in items {
        match item {
            ast::StatementNode::Namespace(namespace_decl) => {
                let declared = name_path_segments(&namespace_decl.name);
                if namespace_decl.items.is_empty() {
                    namespace = declared;
                }
                else {
                    // 带 body 的 `namespace` 以自身路径作为条目 namespace（与 HIR lowering 一致）。
                    collect_items(builder, &declared, &namespace_decl.items);
                }
            }
            other => collect_item(builder, &namespace, other),
        }
    }
}

fn collect_item(builder: &mut FileBuilder, namespace: &[String], item: &ast::StatementNode) {
    match item {
        ast::StatementNode::Using(using) => {
            let mut references = References::default();
            collect_using(using, &mut references);
            builder.register_module_references(namespace, references);
        }
        ast::StatementNode::Class(class) => {
            let mut signature = References::default();
            collect_signature_fields(&class.body.fields, &mut signature);
            builder.register(namespace, DeclarationKind::Class, &class.name.name, None, signature);
            collect_members(builder, namespace, &class.name.name, &class.body.methods);
        }
        ast::StatementNode::Structure(structure) => {
            let mut signature = References::default();
            collect_signature_fields(&structure.body.fields, &mut signature);
            builder.register(namespace, DeclarationKind::Structure, &structure.name.name, None, signature);
            collect_members(builder, namespace, &structure.name.name, &structure.body.methods);
        }
        ast::StatementNode::Trait(trait_decl) => {
            let mut signature = References::default();
            collect_signature_fields(&trait_decl.body.fields, &mut signature);
            builder.register(namespace, DeclarationKind::Trait, &trait_decl.name.name, None, signature);
            collect_members(builder, namespace, &trait_decl.name.name, &trait_decl.body.methods);
        }
        ast::StatementNode::Union(union_def) => {
            let mut signature = References::default();
            collect_signature_fields(&union_def.body.fields, &mut signature);
            builder.register(namespace, DeclarationKind::Union, &union_def.name.name, None, signature);
        }
        ast::StatementNode::Flags(flags) => {
            builder.register(namespace, DeclarationKind::Flags, &flags.name.name, None, References::default());
        }
        ast::StatementNode::Enums(enums) => {
            let key = builder.register(namespace, DeclarationKind::Enum, &enums.name.name, None, References::default());
            for variant in &enums.variants {
                builder.variant_members.push((variant.name.name.clone(), key.clone()));
            }
        }
        ast::StatementNode::Imply(imply) => collect_imply(builder, namespace, imply),
        ast::StatementNode::Micro(micro) => {
            let mut references = References::default();
            collect_micro(micro, &mut references);
            builder.register(namespace, DeclarationKind::Micro, &micro.name.name, None, references);
        }
        ast::StatementNode::TypeAlias(alias) => {
            let mut references = References::default();
            collect_type_expression(&alias.target, &mut references);
            builder.register(namespace, DeclarationKind::TypeAlias, &alias.name.name, None, references);
        }
        ast::StatementNode::TypeFunction(type_function) => {
            let mut references = References::default();
            collect_type_function(type_function, &mut references);
            builder.register(namespace, DeclarationKind::TypeFunction, &type_function.name.name, None, references);
        }
        ast::StatementNode::Statement(inner) => collect_item(builder, namespace, inner),
        ast::StatementNode::Let(let_stmt) => {
            let mut references = References::default();
            collect_let(let_stmt, &mut references);
            builder.register_module_references(namespace, references);
        }
        ast::StatementNode::ExprStmt(expr_stmt) => {
            let mut references = References::default();
            collect_term(&expr_stmt.expr, &mut references);
            builder.register_module_references(namespace, references);
        }
        _ => {}
    }
}

fn collect_members(builder: &mut FileBuilder, namespace: &[String], owner: &str, methods: &[ast::MethodDeclaration]) {
    for method in methods {
        let mut references = References::default();
        collect_method(method, &mut references);
        builder.register(namespace, DeclarationKind::Method, &method.name.name, Some(owner), references);
    }
}

fn collect_imply(builder: &mut FileBuilder, namespace: &[String], imply: &ast::ImplyDeclaration) {
    let mut signature = References::default();
    collect_type_expression(&imply.target_type, &mut signature);
    if let Some(trait_type) = &imply.trait_type {
        collect_type_expression(trait_type, &mut signature);
    }
    // impl 块外壳（目标类型 + trait 路径）始终物化，其引用必须随文件进入闭包。
    builder.register_shell_references(namespace, &signature);
    if imply.methods.is_empty() {
        builder.register_module_references(namespace, signature);
        return;
    }
    for method in &imply.methods {
        // imply 的目标类型与 trait 引用附在每个 witness 方法上：需求任一 witness 都需要它们可解析。
        let mut references = signature.clone();
        collect_method(method, &mut references);
        builder.register(namespace, DeclarationKind::ImplyMethod, &method.name.name, None, references);
    }
}

fn collect_using(using: &ast::UsingDeclaration, references: &mut References) {
    references.imports.insert(using.path.parts.iter().map(|part| part.name.as_str()).collect::<Vec<_>>().join("::"));
    references.paths.insert(name_path_segments(&using.path));
    for import in &using.imports {
        references.paths.insert(vec![import.name.clone()]);
    }
    if let Some(alias) = &using.alias {
        references.paths.insert(vec![alias.name.clone()]);
    }
}

fn collect_signature_fields(fields: &[ast::FieldDeclaration], references: &mut References) {
    for field in fields {
        collect_type_expression(&field.typing, references);
    }
}

fn collect_micro(micro: &ast::MicroDeclaration, references: &mut References) {
    for param in &micro.params {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &micro.return_type {
        collect_type_expression(return_type, references);
    }
    collect_where_constraints(&micro.where_constraints, references);
    collect_block(&micro.body, references);
}

fn collect_method(method: &ast::MethodDeclaration, references: &mut References) {
    for param in &method.parameters {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &method.return_type {
        collect_type_expression(return_type, references);
    }
    collect_where_constraints(&method.where_constraints, references);
    if let Some(body) = &method.body {
        collect_block(body, references);
    }
}

fn collect_type_function(type_function: &ast::TypeFunction, references: &mut References) {
    for param in &type_function.params {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &type_function.return_type {
        collect_type_expression(return_type, references);
    }
    collect_where_constraints(&type_function.where_constraints, references);
    collect_block(&type_function.body, references);
}

/// `where` 约束的受约束类型、trait 路径、类型实参与关联等式右侧都要进入闭包：
/// 它们会被 HIR 冻结成 trait declaration identity，缺一就会出现「trait 约束未解析」。
fn collect_where_constraints(constraints: &[ast::type_nodes::WhereConstraint], references: &mut References) {
    for constraint in constraints {
        collect_type_expression(&constraint.target, references);
        for bound in &constraint.bounds {
            collect_name_path(&bound.trait_path, references);
            for argument in &bound.type_arguments {
                collect_type_expression(argument, references);
            }
            for equation in &bound.associated_types {
                collect_type_expression(&equation.ty, references);
            }
        }
    }
}

fn collect_let(let_stmt: &ast::Let, references: &mut References) {
    if let Some(ty) = &let_stmt.ty {
        collect_type_expression(ty, references);
    }
    if let Some(expr) = &let_stmt.expr {
        collect_term(expr, references);
    }
}

fn collect_block(block: &ast::Block, references: &mut References) {
    for statement in &block.statements {
        collect_statement(statement, references);
    }
}

fn collect_statement(statement: &ast::Statement, references: &mut References) {
    match statement {
        ast::Statement::Let(let_stmt) => collect_let(let_stmt, references),
        ast::Statement::Expression(expr_stmt) => collect_term(&expr_stmt.expr, references),
        _ => {}
    }
}

fn collect_term(term: &ast::TermExpression, references: &mut References) {
    match term {
        ast::TermExpression::ArrayLiteral { items, .. } | ast::TermExpression::TupleLiteral { items, .. } => {
            for item in items {
                collect_term(item, references);
            }
        }
        ast::TermExpression::Unary(node) => collect_term(&node.base, references),
        ast::TermExpression::Binary(node) => {
            collect_term(&node.lhs, references);
            collect_term(&node.rhs, references);
        }
        ast::TermExpression::Cast { expr, ty, .. } => {
            collect_term(expr, references);
            collect_type_expression(ty, references);
        }
        ast::TermExpression::Turbofish { expr, arguments, .. } => {
            collect_term(expr, references);
            for argument in arguments {
                collect_type_expression(argument, references);
            }
        }
        ast::TermExpression::ApplyCall { callee, args, .. } => {
            collect_call_target(callee, references);
            for arg in args {
                collect_term(arg, references);
            }
        }
        ast::TermExpression::DotCall { receiver, field, .. } => {
            collect_term(receiver, references);
            references.paths.insert(vec![field.name.clone()]);
        }
        ast::TermExpression::Index { receiver, index, .. } => {
            collect_term(receiver, references);
            collect_term(index, references);
        }
        ast::TermExpression::Offset { receiver, offset, .. } => {
            collect_term(receiver, references);
            collect_term(offset, references);
        }
        ast::TermExpression::Group { expr, .. } => collect_term(expr, references),
        ast::TermExpression::Block(block) => collect_block(block, references),
        ast::TermExpression::Micro(micro) => collect_block(&micro.body, references),
        ast::TermExpression::Object { callee, fields, .. } => {
            collect_term(callee, references);
            for (_, value) in fields {
                if let Some(value) = value {
                    collect_term(value, references);
                }
            }
        }
        ast::TermExpression::If { condition, then_branch, else_branch, .. } => {
            collect_term(condition, references);
            collect_block(then_branch, references);
            if let Some(else_branch) = else_branch {
                collect_block(else_branch, references);
            }
        }
        ast::TermExpression::Match { scrutinee, arms, .. } => {
            collect_term(scrutinee, references);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_term(guard, references);
                }
                collect_term(&arm.body, references);
            }
        }
        ast::TermExpression::NamePath(path) => collect_name_path(path, references),
        ast::TermExpression::Return(node) => {
            if let Some(expr) = &node.base {
                collect_term(expr, references);
            }
        }
        ast::TermExpression::Break(node) => {
            if let Some(expr) = &node.base {
                collect_term(expr, references);
            }
        }
        ast::TermExpression::Continue(_) => {}
        ast::TermExpression::Raise(node) => {
            if let Some(expr) = &node.base {
                collect_term(expr, references);
            }
        }
        ast::TermExpression::Resume(node) => {
            if let Some(expr) = &node.base {
                collect_term(expr, references);
            }
        }
        ast::TermExpression::Loop { condition, body, .. } => {
            if let Some(condition) = condition {
                collect_term(condition, references);
            }
            collect_block(body, references);
        }
        _ => {}
    }
}

fn collect_call_target(callee: &ast::TermExpression, references: &mut References) {
    match callee {
        ast::TermExpression::NamePath(path) => collect_name_path(path, references),
        ast::TermExpression::DotCall { receiver, field, .. } => {
            collect_term(receiver, references);
            references.paths.insert(vec![field.name.clone()]);
        }
        _ => collect_term(callee, references),
    }
}

fn collect_type_expression(ty: &ast::TypeExpression, references: &mut References) {
    match ty {
        ast::TypeExpression::Binary(node) => {
            collect_type_expression(&node.lhs, references);
            collect_type_expression(&node.rhs, references);
        }
        ast::TypeExpression::Unary(node) => collect_type_expression(&node.base, references),
        ast::TypeExpression::Generic(generic) => {
            references.paths.insert(vec![generic.name.name.clone()]);
        }
        ast::TypeExpression::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_type_expression(&element.ty, references);
            }
        }
        ast::TypeExpression::Function(function) => {
            for param in &function.params {
                collect_type_expression(param, references);
            }
            collect_type_expression(&function.return_type, references);
        }
        ast::TypeExpression::Optional(optional) => collect_type_expression(&optional.inner, references),
        ast::TypeExpression::AssociatedType(associated) => {
            references.paths.insert(vec![associated.base.name.clone()]);
            references.paths.insert(vec![associated.name.name.clone()]);
        }
        ast::TypeExpression::QualifiedAssociatedType(qualified) => {
            collect_type_expression(&qualified.ty, references);
            collect_name_path(&qualified.trait_path, references);
            references.paths.insert(vec![qualified.name.name.clone()]);
        }
        ast::TypeExpression::Namepath(path) => collect_name_path(path, references),
        ast::TypeExpression::Apply(apply) => {
            collect_type_expression(&apply.base, references);
            for argument in &apply.arguments {
                collect_type_expression(argument, references);
            }
        }
    }
}

fn collect_name_path(path: &ast::NamePath, references: &mut References) {
    let segments = name_path_segments(path);
    if segments.is_empty() {
        return;
    }
    if segments.len() == 1 && is_builtin_symbol(&segments[0]) {
        return;
    }
    references.paths.insert(segments);
}

fn is_builtin_symbol(name: &str) -> bool {
    matches!(
        name,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "bool"
            | "utf8"
            | "void"
            | "mut"
            | "self"
            | "Self"
            | "true"
            | "false"
            | "None"
            | "Some"
            | "Fine"
            | "Fail"
    )
}
