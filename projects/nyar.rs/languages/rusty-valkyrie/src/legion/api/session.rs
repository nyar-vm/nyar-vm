//! 批次内复用状态的 session 骨架（增量缓存后续填充）。

use crate::legion::{
    api::request::PlanRequest,
    workspace::planner::{PlannerError, WorkspaceResolver},
};
use nyar::CanonicalTarget;
use std::path::PathBuf;

/// 一次测试批次或 IDE 会话内可复用的解析状态。
#[derive(Debug)]
pub struct CompilerSession {
    /// 已发现的 workspace 根。
    pub workspace_root: PathBuf,
    /// 默认目标（可在单次 `plan` 请求中覆盖）。
    pub target: CanonicalTarget,
    resolver: WorkspaceResolver,
}

impl CompilerSession {
    /// 从首个 `PlanRequest` 发现 workspace 并缓存 resolver。
    pub fn open(request: &PlanRequest) -> Result<Self, PlannerError> {
        let project_input = crate::legion::cmds::project_input::resolve_project_path(&request.project_dir)?;
        let resolver = WorkspaceResolver::discover_for_project(&project_input)?;
        Ok(Self { workspace_root: resolver.root_dir.clone(), target: request.target.clone(), resolver })
    }

    /// 复用已发现的 resolver 生成 plan。
    pub fn plan(&self, request: PlanRequest) -> Result<super::plan::LegionBuildPlan, PlannerError> {
        let project_input = crate::legion::cmds::project_input::resolve_project_path(&request.project_dir)?;
        let build_request = crate::legion::workspace::planner::BuildRequest {
            project_dir: project_input,
            target: request.target,
            output_dir: request.output_dir,
        };
        let inner = self.resolver.build_plan(&build_request)?;
        Ok(super::plan::LegionBuildPlan::from_build_plan(inner))
    }
}
