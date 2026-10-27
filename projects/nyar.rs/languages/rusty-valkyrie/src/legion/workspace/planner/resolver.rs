use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use nyar_package_manager::PackageManager as PackageLegion;

use crate::legion::workspace::manifest::{BuildTargetSpec, DependencySpec, ProjectManifest, WorkspaceManifest};

use super::{
    dependencies::{
        ResolvedDependencySource, default_dist_output_dir, effective_dependency_spec, merged_local_dependency_overrides,
        resolve_dependency_source, select_build_target,
    },
    host::collect_host_inventory,
    source_file_conditions::{filter_semantic_source_groups_for_target, filter_source_files_for_target},
    sources::{
        MAX_SEMANTIC_GROUP_DEPTH, MAX_SEMANTIC_GROUPS, MAX_SOURCE_CLOSURE_FILES, add_distribution_default_dependencies, canonicalize_lossy,
        collect_source_files, collect_test_v_files, dependency_supports_source_context, find_workspace_root, implicit_sdk_dependencies,
        project_visit_key, register_workspace_members, resolve_project_root, same_path, search_start_dir,
    },
    types::{
        BuildPlan, BuildRequest, PlannedDependency, PlannedProject, PlannedSemanticSourceGroup, PlannerError, PlannerResult,
        ProjectResolutionMode, WorkspaceResolver,
    },
};

impl WorkspaceResolver {
    /// 从 `start` 向上查找 workspace 根，并加载其全部成员工程清单。
    pub fn discover(start: impl AsRef<Path>) -> PlannerResult<Self> {
        let start = start.as_ref();
        let workspace_root = find_workspace_root(start).ok_or_else(|| PlannerError::MissingWorkspace(start.to_path_buf()))?;
        let workspace_manifest_path = workspace_root.join("legions.von");
        let workspace_manifest = WorkspaceManifest::parse(&fs::read_to_string(&workspace_manifest_path)?)?;

        let mut projects = BTreeMap::new();
        let mut projects_by_name = BTreeMap::new();
        register_workspace_members(&workspace_root, &workspace_manifest, &mut projects, &mut projects_by_name)?;

        Ok(Self { root_dir: workspace_root, workspace_manifest: Some(workspace_manifest), projects, projects_by_name, single_script: None })
    }

    fn workspace_from_single_script(ctx: crate::legion::workspace::script::SingleScriptContext) -> PlannerResult<Self> {
        let project_dir = ctx.project_dir.clone();
        let mut projects = BTreeMap::new();
        let canonical = canonicalize_lossy(&project_dir);
        projects.insert(canonical.clone(), ctx.manifest.clone());
        let mut projects_by_name = BTreeMap::new();
        projects_by_name.insert(ctx.manifest.name.clone(), canonical);
        Ok(Self { root_dir: project_dir, workspace_manifest: None, projects, projects_by_name, single_script: Some(ctx) })
    }

    /// 解析 `start` 所在工程（workspace 成员、独立包或单脚本）。
    pub fn discover_for_project(start: impl AsRef<Path>) -> PlannerResult<Self> {
        let start = start.as_ref();
        if let Some(workspace_root) = find_workspace_root(start) {
            let mut workspace = Self::discover(workspace_root)?;
            let project_dir = resolve_project_root(start).unwrap_or_else(|| search_start_dir(start));
            if workspace.project_manifest(&project_dir).is_some() {
                return Ok(workspace);
            }
            // 题级 / 示例包常有独立 `legion.von` 且不在 `legions.von` members 中，
            // 但仍通过 `core: true` / `std: true` 引用 workspace 成员；保留已发现的 workspace。
            let manifest_path = project_dir.join("legion.von");
            if manifest_path.is_file() {
                let manifest = ProjectManifest::parse(&fs::read_to_string(&manifest_path)?)?;
                let project_name = manifest.name.clone();
                let canonical = canonicalize_lossy(&project_dir);
                workspace.projects.insert(canonical.clone(), manifest);
                workspace.projects_by_name.insert(project_name, canonical);
                return Ok(workspace);
            }
        }

        if crate::legion::workspace::script::is_script_path(start) {
            if let Some(ctx) = crate::legion::workspace::script::try_load_single_script(start).map_err(PlannerError::Script)? {
                return Self::workspace_from_single_script(ctx);
            }
        }

        let project_dir = resolve_project_root(start).ok_or_else(|| PlannerError::MissingProjectManifest(search_start_dir(start)))?;

        let sidecar = project_dir.join("solution.v");
        if sidecar.is_file() {
            if let Some(ctx) = crate::legion::workspace::script::try_load_single_script(&sidecar).map_err(PlannerError::Script)? {
                return Self::workspace_from_single_script(ctx);
            }
        }

        let manifest_path = project_dir.join("legion.von");
        let manifest = ProjectManifest::parse(&fs::read_to_string(&manifest_path)?)?;
        let canonical = canonicalize_lossy(&project_dir);
        let mut projects = BTreeMap::new();
        projects.insert(canonical.clone(), manifest.clone());
        let mut projects_by_name = BTreeMap::new();
        projects_by_name.insert(manifest.name.clone(), canonical);

        Ok(Self { root_dir: project_dir.clone(), workspace_manifest: None, projects, projects_by_name, single_script: None })
    }

    /// 返回指定目录对应的工程清单（按路径等价匹配）。
    pub fn project_manifest(&self, project_dir: &Path) -> Option<&ProjectManifest> {
        self.projects
            .get(project_dir)
            .or_else(|| self.projects.iter().find(|(candidate, _)| same_path(candidate, project_dir)).map(|(_, manifest)| manifest))
    }

    /// 返回 workspace 中所有成员项目的 manifest 目录（有序）。
    pub fn member_manifest_dirs(&self) -> Vec<PathBuf> {
        self.projects.keys().cloned().collect()
    }

    /// `legions.von` 的 `members` 列表对应的包目录（不含 `discover_for_project` 为依赖解析临时注入的包）。
    ///
    /// 存在 `legions.von` 时，批量构建只遍历这些成员（例如 `leetcode.v`），不含 `discover_for_project` 临时注入的包。
    pub fn legions_member_dirs(&self) -> Vec<PathBuf> {
        let Some(workspace_manifest) = &self.workspace_manifest else {
            return self.projects.keys().cloned().collect();
        };
        let mut dirs = Vec::new();
        for member in &workspace_manifest.members {
            let member_dir = canonicalize_lossy(&self.root_dir.join(member));
            if self.projects.iter().any(|(path, _)| same_path(path, &member_dir)) {
                dirs.push(member_dir);
            }
        }
        dirs.sort();
        dirs.dedup_by(|left, right| same_path(left, right));
        dirs
    }

    /// 发行版默认携带的 SDK：由 planner 注入当前构建的有效依赖闭包。
    ///
    /// 只识别 `sdk_vendor.kind == "distribution-default"` 元数据；不按包名、
    /// 目录名前缀或 target 名称猜测。`std` 不再反向依赖各 adaptor，adaptor
    /// 自身显式依赖 `std`，因此依赖方向与可见性一致。
    fn distribution_default_sdk_names(&self) -> BTreeSet<String> {
        self.projects
            .values()
            .filter(|manifest| manifest.sdk_vendor.as_ref().and_then(|vendor| vendor.kind.as_deref()) == Some("distribution-default"))
            .map(|manifest| manifest.name.clone())
            .collect()
    }

    /// 当前构建根可注入的发行版默认 SDK。
    ///
    /// 依赖方向只有 adaptor → std → core：把一个（传递地）依赖构建根的 SDK 注入构建根，
    /// 会让源码组既要求自己先于 SDK、又要求 SDK 先于自己。构建 `core` 或 `std` 自身时
    /// 因此不注入这类 SDK；应用构建保持原有闭包。
    fn distribution_defaults_without_entry_cycle(&self, entry_dir: &Path) -> BTreeSet<String> {
        let entry_dir = canonicalize_lossy(entry_dir);
        self.distribution_default_sdk_names().into_iter().filter(|sdk| !self.sdk_reaches_project(sdk, &entry_dir)).collect()
    }

    /// `sdk` 的声明依赖闭包是否包含 `entry_dir` 对应的项目。
    fn sdk_reaches_project(&self, sdk: &str, entry_dir: &Path) -> bool {
        let mut pending = vec![sdk.to_string()];
        let mut seen = BTreeSet::new();
        while let Some(name) = pending.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            let Some(dir) = self.projects_by_name.get(&name).cloned()
            else {
                continue;
            };
            if same_path(&canonicalize_lossy(&dir), entry_dir) {
                return true;
            }
            let Some(manifest) = self.project_manifest(&dir)
            else {
                continue;
            };
            if manifest.auto_link.core {
                pending.push("core".to_string());
            }
            if manifest.auto_link.std {
                pending.push("std".to_string());
            }
            for (dependency_name, spec) in &manifest.dependencies {
                if !matches!(spec, DependencySpec::Disabled) {
                    pending.push(dependency_name.clone());
                }
            }
        }
        false
    }

    /// 构建起始目录是否就是带 `legions.von` 的 workspace 根（在根上 `legion build` → 顺序构建全部 members）。
    pub fn is_at_workspace_root(&self, project_dir: &Path) -> bool {
        self.workspace_manifest.is_some() && same_path(&search_start_dir(project_dir), &self.root_dir)
    }

    /// 仅有 `legions.von`、根目录没有 `legion.von`（纯 workspace 根，无根包）。
    pub fn is_workspace_only_root(&self, project_dir: &Path) -> bool {
        self.is_at_workspace_root(project_dir) && !self.root_dir.join("legion.von").exists()
    }

    /// 按请求生成构建计划：解析构建目标、依赖与源码闭包。
    pub fn build_plan(&self, request: &BuildRequest) -> PlannerResult<BuildPlan> {
        if let Some(ctx) = &self.single_script {
            return self.build_plan_for_manifest(ctx.project_dir.clone(), &ctx.manifest, request);
        }

        let project_dir = if self.workspace_manifest.is_none() {
            self.root_dir.clone()
        }
        else {
            resolve_project_root(&request.project_dir).unwrap_or_else(|| search_start_dir(&request.project_dir))
        };
        let manifest = if self.workspace_manifest.is_none() {
            self.projects.values().next().ok_or_else(|| PlannerError::MissingProjectManifest(project_dir.clone()))?
        }
        else {
            self.project_manifest(&project_dir).ok_or_else(|| PlannerError::MissingProjectManifest(project_dir.clone()))?
        };
        let build_target = select_build_target(manifest, &request.target)
            .ok_or_else(|| PlannerError::MissingBuildTarget { project: manifest.name.clone(), target: request.target })?;

        let dependencies = self.collect_dependencies(&project_dir, manifest, &build_target, true)?;
        let output_dir = request.output_dir.clone().unwrap_or_else(|| default_dist_output_dir(&project_dir, &request.target));

        // 收集源码闭包：包含项目自身及其所有依赖（含传递依赖）的源文件。
        // 这是模块系统的核心：让编译器能看到依赖项目定义的 struct/function/type。
        let mut visited = BTreeSet::new();
        let source_files = self.collect_source_closure(&project_dir, manifest, &build_target, &mut visited, true)?;
        let semantic_source_groups = filter_semantic_source_groups_for_target(
            self.collect_semantic_source_groups(&project_dir, manifest, &build_target)?,
            request.target,
        )?;
        let source_files = filter_source_files_for_target(source_files, request.target)?;
        let host_inventory = host_inventory_for_groups(&semantic_source_groups, request.target, &build_target)?;

        Ok(BuildPlan {
            workspace_root: self.root_dir.clone(),
            output_dir,
            project: PlannedProject {
                name: manifest.name.clone(),
                manifest_dir: project_dir.clone(),
                manifest_path: self
                    .single_script
                    .as_ref()
                    .filter(|ctx| same_path(&ctx.project_dir, &project_dir))
                    .map(|ctx| ctx.script_path.clone())
                    .unwrap_or_else(|| project_dir.join("legion.von")),
                source_files,
                semantic_source_groups,
                host_contracts: host_inventory.contracts,
                host_provider_candidates: host_inventory.providers,
                selected_host_providers: host_inventory.selected_providers,
                build_target,
                artifact_kind: manifest.artifact,
                dependencies,
            },
        })
    }

    /// 构造测试/基准编译计划：始终允许指定 target，并合并 `source/` + `test/` 源文件。
    pub fn build_test_plan(&self, request: &BuildRequest) -> PlannerResult<(BuildPlan, ProjectResolutionMode)> {
        let (project_dir, manifest, mode) = if let Some(ctx) = &self.single_script {
            (ctx.project_dir.clone(), ctx.manifest.clone(), ProjectResolutionMode::Script)
        }
        else {
            let project_dir = resolve_project_root(&request.project_dir).unwrap_or_else(|| search_start_dir(&request.project_dir));
            if let Some(manifest) = self.project_manifest(&project_dir) {
                let mode = if self.workspace_manifest.is_some() { ProjectResolutionMode::Workspace } else { ProjectResolutionMode::Script };
                (project_dir, manifest.clone(), mode)
            }
            else {
                let manifest_path = project_dir.join("legion.von");
                if !manifest_path.exists() {
                    return Err(PlannerError::MissingProjectManifest(project_dir));
                }
                let manifest = ProjectManifest::parse(&fs::read_to_string(&manifest_path)?)?;
                let mode = if self.workspace_manifest.is_some() { ProjectResolutionMode::Package } else { ProjectResolutionMode::Script };
                (project_dir, manifest, mode)
            }
        };

        let mut build_target = select_build_target(&manifest, &request.target)
            .unwrap_or_else(|| BuildTargetSpec { target: request.target.clone(), ..BuildTargetSpec::default() });
        build_target.target = request.target.clone();

        let dependencies = self.collect_dependencies(&project_dir, &manifest, &build_target, true)?;
        let output_dir =
            request.output_dir.clone().unwrap_or_else(|| project_dir.join(".cache").join("test").join(request.target.as_canonical_str()));

        let mut visited = BTreeSet::new();
        let mut source_files = self.collect_source_closure(&project_dir, &manifest, &build_target, &mut visited, true)?;
        let mut test_files = collect_test_v_files(&project_dir)?;
        source_files.append(&mut test_files);
        source_files.sort();
        source_files.dedup();

        let semantic_source_groups = filter_semantic_source_groups_for_target(
            self.collect_semantic_source_groups(&project_dir, &manifest, &build_target)?,
            request.target,
        )?;
        let source_files = filter_source_files_for_target(source_files, request.target)?;
        let host_inventory = host_inventory_for_groups(&semantic_source_groups, request.target, &build_target)?;

        Ok((
            BuildPlan {
                workspace_root: self.root_dir.clone(),
                output_dir,
                project: PlannedProject {
                    name: manifest.name.clone(),
                    manifest_dir: project_dir.clone(),
                    manifest_path: self
                        .single_script
                        .as_ref()
                        .map(|ctx| ctx.script_path.clone())
                        .unwrap_or_else(|| project_dir.join("legion.von")),
                    source_files,
                    semantic_source_groups,
                    host_contracts: host_inventory.contracts,
                    host_provider_candidates: host_inventory.providers,
                    selected_host_providers: host_inventory.selected_providers,
                    build_target,
                    artifact_kind: manifest.artifact,
                    dependencies,
                },
            },
            mode,
        ))
    }

    /// 递归收集源码闭包：项目自身 + 所有依赖（含传递依赖）的源文件。
    ///
    /// 这是模块系统依赖管理的核心实现。通过递归遍历依赖图，
    /// 将所有相关项目的源文件收集到一起，供编译器做合并编译。
    ///
    /// 使用 `visited` 集合防止循环依赖导致的无限递归。
    fn collect_source_closure(
        &self,
        project_dir: &Path,
        manifest: &ProjectManifest,
        build_target: &BuildTargetSpec,
        visited: &mut BTreeSet<String>,
        inject_distribution_defaults: bool,
    ) -> PlannerResult<Vec<PathBuf>> {
        let canonical_dir = canonicalize_lossy(project_dir);
        let visit_key = project_visit_key(&canonical_dir);
        if visited.contains(&visit_key) {
            return Ok(Vec::new());
        }
        visited.insert(visit_key);

        if let Some(ctx) = &self.single_script {
            if same_path(project_dir, &ctx.project_dir) {
                return Ok(vec![ctx.script_path.clone()]);
            }
        }

        // 收集当前项目自身的源文件。target 不参与声明是否存在，只选择 adaptor 与 TargetExecutionPlan。
        let mut all_files = collect_source_files(&canonical_dir)?;

        // 收集当前项目的依赖名称（auto_link + 显式声明 + 平台隐式 SDK）。
        let mut dep_names = BTreeSet::new();
        if manifest.auto_link.core {
            dep_names.insert("core".to_string());
        }
        if manifest.auto_link.std {
            dep_names.insert("std".to_string());
        }
        for (dependency_name, dependency_spec) in &manifest.dependencies {
            if !matches!(dependency_spec, DependencySpec::Disabled) {
                dep_names.insert(dependency_name.clone());
            }
        }
        let implicit_names = implicit_sdk_dependencies(&build_target.publish).into_iter().map(|item| item.to_string()).collect::<BTreeSet<_>>();
        for implicit in &implicit_names {
            dep_names.insert(implicit.clone());
        }
        let distribution_defaults =
            if inject_distribution_defaults { self.distribution_defaults_without_entry_cycle(project_dir) } else { BTreeSet::new() };
        add_distribution_default_dependencies(manifest, &distribution_defaults, &mut dep_names);

        // 递归收集每个依赖的源文件。
        for dep_name in dep_names {
            let local_manifest_dir = self.projects_by_name.get(&dep_name).cloned();
            let dependency_spec = manifest.dependencies.get(&dep_name);
            let allow_missing_when_auto = implicit_names.contains(&dep_name) || distribution_defaults.contains(&dep_name);
            let overrides = merged_local_dependency_overrides(project_dir);
            let (effective_spec, path_base) = effective_dependency_spec(&dep_name, dependency_spec, &overrides, project_dir);
            match resolve_dependency_source(
                self,
                project_dir,
                &path_base,
                effective_spec.as_deref(),
                local_manifest_dir,
                allow_missing_when_auto,
                &manifest.name,
                &dep_name,
            )? {
                ResolvedDependencySource::Local(dep_dir) => {
                    if let Some(dep_manifest) = self.load_project_manifest_from_dir(&dep_dir) {
                        if !dependency_supports_source_context(&dep_name, &dep_manifest, &build_target.target, &build_target.publish) {
                            continue;
                        }
                        let dep_files = self.collect_source_closure(&dep_dir, &dep_manifest, build_target, visited, false)?;
                        all_files.extend(dep_files);
                    }
                }
                ResolvedDependencySource::Registry { version, registry } => {
                    let install_dir =
                        self.install_registry_dependency(self.root_dir.as_path(), &dep_name, &version, &registry, &manifest.name)?;
                    if let Some(dep_manifest) = self.load_project_manifest_from_dir(&install_dir) {
                        if dependency_supports_source_context(&dep_name, &dep_manifest, &build_target.target, &build_target.publish) {
                            let dep_files = self.collect_source_closure(&install_dir, &dep_manifest, build_target, visited, false)?;
                            all_files.extend(dep_files);
                        }
                    }
                    else {
                        all_files.extend(collect_source_files(&install_dir)?);
                    }
                }
                ResolvedDependencySource::MissingImplicit => {}
            }
        }

        // 去重并排序，确保编译顺序稳定。
        all_files.sort();
        all_files.dedup();
        if all_files.len() > MAX_SOURCE_CLOSURE_FILES {
            return Err(PlannerError::ResourceLimitExceeded {
                kind: "source_closure_files",
                limit: MAX_SOURCE_CLOSURE_FILES,
                actual: all_files.len(),
            });
        }
        Ok(all_files)
    }

    fn collect_semantic_source_groups(
        &self,
        project_dir: &Path,
        manifest: &ProjectManifest,
        build_target: &BuildTargetSpec,
    ) -> PlannerResult<Vec<PlannedSemanticSourceGroup>> {
        let mut groups = Vec::new();
        let mut visited = BTreeSet::new();
        self.collect_semantic_source_groups_inner(project_dir, manifest, &manifest.name, build_target, &mut visited, &mut groups, 0)?;
        Ok(groups)
    }

    fn collect_semantic_source_groups_inner(
        &self,
        project_dir: &Path,
        manifest: &ProjectManifest,
        dependency_key: &str,
        build_target: &BuildTargetSpec,
        visited: &mut BTreeSet<String>,
        groups: &mut Vec<PlannedSemanticSourceGroup>,
        depth: usize,
    ) -> PlannerResult<()> {
        if depth > MAX_SEMANTIC_GROUP_DEPTH {
            return Err(PlannerError::ResourceLimitExceeded { kind: "semantic_group_depth", limit: MAX_SEMANTIC_GROUP_DEPTH, actual: depth });
        }
        if groups.len() >= MAX_SEMANTIC_GROUPS {
            return Err(PlannerError::ResourceLimitExceeded { kind: "semantic_groups", limit: MAX_SEMANTIC_GROUPS, actual: groups.len() });
        }
        let manifest_dir = canonicalize_lossy(project_dir);
        let visit_key = project_visit_key(&manifest_dir);
        if !visited.insert(visit_key) {
            return Ok(());
        }
        // 发行版默认 SDK 只注入当前构建根的有效依赖闭包；依赖自身不再携带
        // 反向 adaptor 边，否则 `std` 与其 adaptor 会形成可见性环。
        let dependencies = self.collect_dependencies(project_dir, manifest, build_target, depth == 0)?;
        for dependency in &dependencies {
            let dependency_manifest = self
                .load_project_manifest_from_dir(&dependency.manifest_dir)
                .ok_or_else(|| PlannerError::MissingProjectManifest(dependency.manifest_dir.clone()))?;
            self.collect_semantic_source_groups_inner(
                &dependency.manifest_dir,
                &dependency_manifest,
                &dependency.name,
                build_target,
                visited,
                groups,
                depth + 1,
            )?;
        }
        let source_files = collect_source_files(&manifest_dir)?;
        groups.push(PlannedSemanticSourceGroup {
            dependency_key: dependency_key.to_string(),
            name: manifest.name.clone(),
            manifest_dir,
            source_files,
            direct_dependencies: dependencies.into_iter().map(|dependency| dependency.name).collect(),
            is_entry: depth == 0,
        });
        Ok(())
    }

    fn build_plan_for_manifest(&self, project_dir: PathBuf, manifest: &ProjectManifest, request: &BuildRequest) -> PlannerResult<BuildPlan> {
        let build_target = select_build_target(manifest, &request.target)
            .ok_or_else(|| PlannerError::MissingBuildTarget { project: manifest.name.clone(), target: request.target.clone() })?;

        let dependencies = self.collect_dependencies(&project_dir, manifest, &build_target, true)?;
        let output_dir = request.output_dir.clone().unwrap_or_else(|| default_dist_output_dir(&project_dir, &request.target));

        let mut visited = BTreeSet::new();
        let source_files = self.collect_source_closure(&project_dir, manifest, &build_target, &mut visited, true)?;
        let semantic_source_groups = filter_semantic_source_groups_for_target(
            self.collect_semantic_source_groups(&project_dir, manifest, &build_target)?,
            request.target,
        )?;
        let source_files = filter_source_files_for_target(source_files, request.target)?;
        let host_inventory = host_inventory_for_groups(&semantic_source_groups, request.target, &build_target)?;

        Ok(BuildPlan {
            workspace_root: self.root_dir.clone(),
            output_dir,
            project: PlannedProject {
                name: manifest.name.clone(),
                manifest_dir: project_dir.clone(),
                manifest_path: self
                    .single_script
                    .as_ref()
                    .filter(|ctx| same_path(&ctx.project_dir, &project_dir))
                    .map(|ctx| ctx.script_path.clone())
                    .unwrap_or_else(|| project_dir.join("legion.von")),
                source_files,
                semantic_source_groups,
                host_contracts: host_inventory.contracts,
                host_provider_candidates: host_inventory.providers,
                selected_host_providers: host_inventory.selected_providers,
                build_target,
                artifact_kind: manifest.artifact,
                dependencies,
            },
        })
    }

    fn collect_dependencies(
        &self,
        project_manifest_dir: &Path,
        manifest: &ProjectManifest,
        build_target: &BuildTargetSpec,
        inject_distribution_defaults: bool,
    ) -> PlannerResult<Vec<PlannedDependency>> {
        let mut planned = Vec::new();
        let mut names = BTreeSet::new();
        let mut declared_specs = BTreeMap::new();
        let implicit_names = implicit_sdk_dependencies(&build_target.publish).into_iter().map(|item| item.to_string()).collect::<BTreeSet<_>>();

        if manifest.auto_link.core {
            names.insert("core".to_string());
        }
        if manifest.auto_link.std {
            names.insert("std".to_string());
        }
        for (dependency_name, dependency_spec) in &manifest.dependencies {
            if !matches!(dependency_spec, DependencySpec::Disabled) {
                names.insert(dependency_name.clone());
                declared_specs.insert(dependency_name.clone(), dependency_spec.clone());
            }
        }
        for implicit in &implicit_names {
            names.insert(implicit.clone());
        }
        let distribution_defaults =
            if inject_distribution_defaults { self.distribution_defaults_without_entry_cycle(project_manifest_dir) } else { BTreeSet::new() };
        add_distribution_default_dependencies(manifest, &distribution_defaults, &mut names);

        for dependency_name in names {
            let local_manifest_dir = self.projects_by_name.get(&dependency_name).cloned();
            let dependency_spec = declared_specs.get(&dependency_name);
            let allow_missing_when_auto = implicit_names.contains(&dependency_name) || distribution_defaults.contains(&dependency_name);
            let overrides = merged_local_dependency_overrides(project_manifest_dir);
            let (effective_spec, path_base) = effective_dependency_spec(&dependency_name, dependency_spec, &overrides, project_manifest_dir);

            let manifest_dir = match resolve_dependency_source(
                self,
                project_manifest_dir,
                &path_base,
                effective_spec.as_deref(),
                local_manifest_dir,
                allow_missing_when_auto,
                &manifest.name,
                &dependency_name,
            )? {
                ResolvedDependencySource::Local(manifest_dir) => manifest_dir,
                ResolvedDependencySource::Registry { version, registry } => {
                    self.install_registry_dependency(self.root_dir.as_path(), &dependency_name, &version, &registry, &manifest.name)?
                }
                ResolvedDependencySource::MissingImplicit => continue,
            };
            if let Some(dependency_manifest) = self.project_manifest(&manifest_dir) {
                if !dependency_supports_source_context(&dependency_name, dependency_manifest, &build_target.target, &build_target.publish) {
                    continue;
                }
            }
            planned.push(PlannedDependency { name: dependency_name, manifest_dir });
        }

        planned.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(planned)
    }

    fn registry_vendor_path(&self, dependency_name: &str, version: &str, registry: &str) -> PathBuf {
        self.root_dir.join("vendors").join(registry).join(format!("{dependency_name}@{version}"))
    }

    fn load_project_manifest_from_dir(&self, project_dir: &Path) -> Option<ProjectManifest> {
        self.project_manifest(project_dir).cloned().or_else(|| {
            let manifest_path = project_dir.join("legion.von");
            if !manifest_path.exists() {
                return None;
            }
            let source = fs::read_to_string(&manifest_path).ok()?;
            ProjectManifest::parse(&source).ok()
        })
    }

    fn install_registry_dependency(
        &self,
        project_dir: &Path,
        dependency_name: &str,
        version: &str,
        registry: &str,
        project_name: &str,
    ) -> PlannerResult<PathBuf> {
        let vendor_path = self.registry_vendor_path(dependency_name, version, registry);
        if vendor_path.join("legion.von").is_file() {
            return Ok(vendor_path);
        }

        let mut legion = PackageLegion::open(project_dir, crate::legion::workspace::LEGION_PROJECT_LAYOUT).map_err(|error| {
            PlannerError::RegistryDependencyInstallFailed {
                project: project_name.to_string(),
                dependency: dependency_name.to_string(),
                version: version.to_string(),
                registry: registry.to_string(),
                reason: error.to_string(),
            }
        })?;
        let info =
            legion.install_one(dependency_name, version, registry, false).map_err(|error| PlannerError::RegistryDependencyInstallFailed {
                project: project_name.to_string(),
                dependency: dependency_name.to_string(),
                version: version.to_string(),
                registry: registry.to_string(),
                reason: error.to_string(),
            })?;
        info.install_path.map(PathBuf::from).ok_or_else(|| PlannerError::RegistryDependencyInstallFailed {
            project: project_name.to_string(),
            dependency: dependency_name.to_string(),
            version: version.to_string(),
            registry: registry.to_string(),
            reason: "missing install path".to_string(),
        })
    }
}

fn host_inventory_for_groups(
    semantic_source_groups: &[PlannedSemanticSourceGroup],
    target: nyar::CanonicalTarget,
    build_target: &BuildTargetSpec,
) -> PlannerResult<super::host::HostInventory> {
    let arch = super::compile_arch_key(&build_target.target);
    let demanded = super::demanded_declaration_keys(semantic_source_groups, arch)?;
    collect_host_inventory(semantic_source_groups, target, demanded.as_ref())
}
