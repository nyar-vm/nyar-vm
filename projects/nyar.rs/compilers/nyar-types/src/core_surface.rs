//! Versioned, closed core-surface registry.

/// A language construct admitted by a core-surface manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreFeature {
    /// 包级 `import` 与模块边界。
    PackageImports,
    /// 具名函数与可调用项。
    Functions,
    /// 条件、循环与结构化控制流。
    ControlFlow,
    /// 名义 ADT（结构体、枚举、trait 声明）。
    NominalTypes,
    /// 显式可空类型（非名义 `Option`）。
    Nullable,
    /// 名义 `Option` 类型族。
    Option,
    /// UTF-8 文本类型与字面量。
    Utf8,
    /// UTF-16 文本类型与字面量。
    Utf16,
    /// 64 位浮点标量。
    Float64,
    /// 带显式类型的变量与参数声明。
    TypedDeclarations,
}

/// Explicit versioned allow-list. Unknown constructs must be rejected by validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSurfaceManifest {
    /// 核心曲面版本字符串（非空）。
    pub version: String,
    features: Vec<CoreFeature>,
}

impl CoreSurfaceManifest {
    /// Creates a manifest; an empty version or feature list is invalid.
    pub fn new(version: impl Into<String>, features: impl IntoIterator<Item = CoreFeature>) -> Result<Self, &'static str> {
        let manifest = Self { version: version.into(), features: features.into_iter().collect() };
        if manifest.version.trim().is_empty() || manifest.features.is_empty() {
            return Err("core surface manifest is incomplete");
        }
        Ok(manifest)
    }
    /// Returns whether a feature is explicitly admitted.
    pub fn admits(&self, feature: CoreFeature) -> bool {
        self.features.contains(&feature)
    }
    /// Rejects a construct outside the versioned core surface.
    pub fn require(&self, feature: CoreFeature) -> Result<(), &'static str> {
        self.admits(feature).then_some(()).ok_or("construct is outside the core surface")
    }
}
