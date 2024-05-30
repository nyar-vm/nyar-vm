use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use nyar_package_registry::{Package, Registry};

use crate::{DependencySpec, PackageManagerError, Result};

/// 依赖解析树中的一个节点，包含注册表元数据与子依赖。
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// 包名。
    pub name: String,
    /// 解析得到的版本。
    pub version: String,
    /// 提供该包的注册表名称。
    pub registry: String,
    /// 注册表返回的完整 [`Package`] 元数据。
    pub package: Package,
    /// 递归解析的直接依赖。
    pub children: Vec<DependencyNode>,
}

/// 递归依赖解析器（有深度上限，不做完整 SAT）。
pub struct DependencyResolver {
    registries: HashMap<String, Arc<dyn Registry>>,
    max_depth: usize,
}

impl DependencyResolver {
    /// 绑定可用注册表适配器；默认最大递归深度为 32。
    pub fn new(registries: HashMap<String, Arc<dyn Registry>>) -> Self {
        Self { registries, max_depth: 32 }
    }

    /// 解析 manifest 中的全部非 workspace 依赖，返回森林根节点列表。
    pub fn resolve_all(&self, dependencies: &BTreeMap<String, DependencySpec>, default_registry: &str) -> Result<Vec<DependencyNode>> {
        let mut visiting = BTreeSet::new();
        let mut resolved = BTreeMap::new();
        let mut out = Vec::new();
        for (name, spec) in dependencies {
            if spec.is_workspace() {
                continue;
            }
            let version = spec.version_constraint().unwrap_or("latest");
            let registry_name = match spec {
                DependencySpec::Detailed { registry: Some(registry), .. } => registry.as_str(),
                _ => default_registry,
            };
            out.push(self.resolve_one(name, version, registry_name, 0, &mut visiting, &mut resolved)?);
        }
        Ok(out)
    }

    /// 解析单个包及其传递依赖，检测环与深度溢出。
    pub fn resolve_one(
        &self,
        name: &str,
        version: &str,
        registry_name: &str,
        depth: usize,
        visiting: &mut BTreeSet<String>,
        resolved: &mut BTreeMap<String, DependencyNode>,
    ) -> Result<DependencyNode> {
        if depth > self.max_depth {
            return Err(PackageManagerError::message(format!("dependency depth exceeded for {name}")));
        }
        if let Some(node) = resolved.get(name) {
            return Ok(node.clone());
        }
        if !visiting.insert(name.to_string()) {
            return Err(PackageManagerError::message(format!("dependency cycle detected at {name}")));
        }
        let registry =
            self.registries.get(registry_name).ok_or_else(|| PackageManagerError::message(format!("注册器 {registry_name} 未找到")))?;
        let package = registry.get_package(name, version)?;
        let mut children = Vec::new();
        for (child_name, child_version) in &package.dependency_versions {
            children.push(self.resolve_one(child_name, child_version, registry_name, depth + 1, visiting, resolved)?);
        }
        visiting.remove(name);
        let node = DependencyNode {
            name: package.name.clone(),
            version: package.version.clone(),
            registry: registry_name.to_string(),
            package,
            children,
        };
        resolved.insert(name.to_string(), node.clone());
        Ok(node)
    }
}
