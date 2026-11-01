//! Asgard 交付宿主策略与 chunk 分割规划。
//!
//! 浏览器 HTTP 交付与桌面 WebView 交付共享同一套 DOM 执行语义、减枝规则和
//! JS/Wasm ABI，但产物组织不同：
//! - 浏览器按语义可达性减枝后，还需要计算 chunk 分割方案并生成 loader/manifest；
//! - 桌面 WebView 同样减枝，但直接在 target 策略中跳过 chunk partitioning，
//!   生成未分割的执行闭包，不计算浏览器 chunk 拓扑，也不生成动态 loader/manifest。
//!
//! 本模块只拥有这条策略事实，不生成文件名、hash 或 import 路径；具体产物由
//! 后续构建阶段消费 [`AsgardBuildPlan`]。

use serde::Serialize;

use crate::asgard::{
    config_model::{ChunkBoundary, ChunkBudgets, ChunkPlan, ResolvedAsgardConfig},
    error::{AsgardError, AsgardResult},
};

/// 交付宿主策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryHost {
    /// 浏览器 HTTP 交付：按语义减枝后计算 chunk 分割方案。
    BrowserHttp,
    /// 桌面 WebView 交付：按语义减枝后直接生成未分割的执行闭包。
    DesktopWebView,
}

/// 语义减枝策略；所有宿主都必须按可达性减枝，不能携带整棵 std 或未使用组件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PruningStrategy {
    /// 按语义可达性减枝（函数、类型、数据段、组件、glue 与宿主绑定）。
    SemanticReachability,
}

/// chunk 分割规划结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkPartitioning {
    /// 浏览器交付：保留作者配置的边界与预算，供 loader/manifest 生成。
    Partitioned(ChunkPlan),
    /// 桌面交付：跳过分割规划；不产生边界、chunk 拓扑或动态 loader。
    Skipped,
}

impl ChunkPartitioning {
    /// 是否进行了 chunk 分割规划。
    pub fn is_partitioned(&self) -> bool {
        matches!(self, Self::Partitioned(_))
    }

    /// 浏览器分割计划；桌面未分割交付返回 `None`。
    pub fn plan(&self) -> Option<&ChunkPlan> {
        match self {
            Self::Partitioned(plan) => Some(plan),
            Self::Skipped => None,
        }
    }

    /// 规划出的切分边界；桌面未分割交付为空。
    pub fn boundaries(&self) -> &[ChunkBoundary] {
        self.plan().map_or(&[], |plan| plan.boundaries.as_slice())
    }

    /// 分包预算；桌面未分割交付不承担 chunk 预算门禁，返回 `None`。
    pub fn budgets(&self) -> Option<&ChunkBudgets> {
        self.plan().map(|plan| &plan.budgets)
    }

    /// 是否需要生成动态 chunk loader/manifest；桌面未分割交付不需要。
    pub fn requires_chunk_loader(&self) -> bool {
        self.is_partitioned()
    }
}

/// 一次 Asgard 构建的目标策略快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsgardBuildPlan {
    /// 交付宿主。
    pub host: DeliveryHost,
    /// 减枝策略；所有宿主一致。
    pub pruning: PruningStrategy,
    /// chunk 分割规划结果。
    pub chunk_partitioning: ChunkPartitioning,
}

/// 按交付宿主决定 chunk 分割规划。
///
/// 桌面 WebView 无论配置里写了多少 `artifact.chunks.boundaries`，都跳过分割规划；
/// 忽略这些边界不是丢事实，而是该宿主的产品合同。浏览器交付保留配置计划。
pub fn plan_asgard_build(host: DeliveryHost, chunks: &ChunkPlan) -> AsgardBuildPlan {
    let chunk_partitioning = match host {
        DeliveryHost::BrowserHttp => ChunkPartitioning::Partitioned(chunks.clone()),
        DeliveryHost::DesktopWebView => ChunkPartitioning::Skipped,
    };
    AsgardBuildPlan { host, pruning: PruningStrategy::SemanticReachability, chunk_partitioning }
}

impl ResolvedAsgardConfig {
    /// 以已校验配置生成目标策略快照；不读取配置以外的工程事实。
    pub fn build_plan(&self, host: DeliveryHost) -> AsgardBuildPlan {
        plan_asgard_build(host, &self.chunks)
    }
}

/// 浏览器交付的 chunk 依赖图：loader 按需取 chunk 时消费的唯一数据合同。
///
/// 本合同只冻结图形状：入口 chunk（首屏闭包）与按需 chunk 节点、依赖边和
/// preload/prefetch 策略位。真实 chunk 产物（wasm 分割、URL/hash、import/export
/// 签名与初始化顺序）由 Compiler/backend 生产后按本合同写入 manifest；在产物
/// 链接通之前，构建路径经 [`ensure_browser_chunk_delivery_supported`] 失败闭合，
/// 不得以单 chunk 产物冒充已分割交付。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChunkGraph {
    /// 入口 chunk 标识（首屏闭包所在 chunk 的名字）。
    pub entry: String,
    /// 按需 chunk 节点；顺序即 `artifact.chunks.boundaries` 声明顺序，保持确定性。
    pub nodes: Vec<ChunkGraphNode>,
}

/// 一个按需 chunk 节点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChunkGraphNode {
    /// 边界对应的组件标识，同时是该按需 chunk 的名字。
    pub component: String,
    /// 该 chunk 的依赖（loader 必须先取回并初始化的 chunk 名）。
    ///
    /// 当前只承载入口闭包边：每个按需 chunk 至少依赖入口 chunk。组件之间的
    /// 依赖边需要 frontend 生产的组件依赖事实，在事实接通前不得臆造。
    pub depends_on: Vec<String>,
    /// 入口 chunk 是否预加载该边界（对应 `artifact.chunks.boundaries[].preload`）。
    pub preload: bool,
    /// 已知下一步是否预取（对应 `artifact.chunks.boundaries[].prefetch`）。
    pub prefetch: bool,
}

/// 把分割计划解析为 loader 依赖图；不生成文件名、hash 或 import 路径。
///
/// 拒绝使图不可判定的输入：重复的边界组件、把入口组件又声明为按需边界。
/// 入口组件名为空同样拒绝，图必须锚定一个真实首屏闭包。
pub fn resolve_chunk_graph(plan: &ChunkPlan, entry_component: &str) -> AsgardResult<ChunkGraph> {
    if entry_component.is_empty() {
        return Err(AsgardError::msg(report!("chunk 依赖图的入口组件名不能为空")));
    }
    let mut nodes = Vec::with_capacity(plan.boundaries.len());
    for boundary in &plan.boundaries {
        if boundary.component == entry_component {
            return Err(AsgardError::msg(report!("chunk 边界 `{entry_component}` 与入口组件重复：入口属于首屏闭包，不是按需边界")));
        }
        if nodes.iter().any(|node: &ChunkGraphNode| node.component == boundary.component) {
            return Err(AsgardError::msg(report!("chunk 边界组件 `{}` 重复声明，依赖图节点必须唯一", boundary.component)));
        }
        nodes.push(ChunkGraphNode {
            component: boundary.component.clone(),
            depends_on: vec![entry_component.to_string()],
            preload: boundary.preload,
            prefetch: boundary.prefetch,
        });
    }
    Ok(ChunkGraph { entry: entry_component.to_string(), nodes })
}

/// 浏览器构建消费分割计划前的失败闭合门。
///
/// 声明了 `artifact.chunks.boundaries` 即要求真实多 chunk 产物与依赖 loader；
/// 实际 JS/Wasm chunk 生成尚未实施，任何带边界的浏览器计划在此明确失败，
/// 不静默回落为单 chunk 交付。未声明边界或桌面未分割交付直接放行。
pub fn ensure_browser_chunk_delivery_supported(plan: &AsgardBuildPlan) -> AsgardResult<()> {
    match &plan.chunk_partitioning {
        ChunkPartitioning::Skipped => Ok(()),
        ChunkPartitioning::Partitioned(chunks) if chunks.boundaries.is_empty() => Ok(()),
        ChunkPartitioning::Partitioned(chunks) => Err(AsgardError::msg(report!(
            "浏览器交付已声明 {} 条 chunk 边界，但实际 JS/Wasm chunk 生成与依赖 loader 尚未实施；构建在此失败闭合，不以单 chunk 产物冒充分割交付",
            chunks.boundaries.len()
        ))),
    }
}
