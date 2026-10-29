//! 批次内复用状态的 session 骨架（增量缓存后续填充）。

use crate::legion::{
    api::{
        plan::LegionBuildPlan,
        request::PlanRequest,
        test::{LegionTestReport, LegionTestSelector},
    },
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
    pub fn plan(&self, request: PlanRequest) -> Result<LegionBuildPlan, PlannerError> {
        let project_input = crate::legion::cmds::project_input::resolve_project_path(&request.project_dir)?;
        let build_request = crate::legion::workspace::planner::BuildRequest {
            project_dir: project_input,
            target: request.target,
            output_dir: request.output_dir,
            ..Default::default()
        };
        let inner = self.resolver.build_plan(&build_request)?;
        Ok(LegionBuildPlan::from_build_plan(inner))
    }

    /// 在同一会话内规划并编译（LeetCode 等场景：一次 open、多次 plan/compile）。
    pub fn compile(
        &self,
        request: PlanRequest,
        options: super::request::CompileOptions,
    ) -> Result<super::compile::LegionCompileReport, crate::legion::cmds::error::LegionError> {
        let plan = self.plan(request).map_err(crate::legion::cmds::error::LegionError::Planner)?;
        super::compile::compile(&plan, options)
    }

    /// 在同一会话内规划并运行测试。
    pub fn test(
        &self,
        request: PlanRequest,
        selector: LegionTestSelector,
    ) -> Result<LegionTestReport, crate::legion::cmds::error::LegionError> {
        let plan = self.plan(request).map_err(crate::legion::cmds::error::LegionError::Planner)?;
        super::test::test_project(&plan, selector)
    }
}
