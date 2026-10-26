#![doc = include_str!("readme.md")]
#![deny(missing_docs)]

pub mod api;
pub mod artifact_formats;
pub mod cache;
pub mod cmds;
pub mod forward;
pub mod unity_export;
pub mod von;
pub mod workspace;

pub use api::{
    CompileOptions, CompilerSession, LegionBuildPlan, LegionCompileReport, LegionDiagnostic, LegionSemanticGroupSummary, LegionStage,
    LegionStageStatus, PlanRequest, compile, compile_project, plan,
};
pub use artifact_formats::{artifact_format_from_extension, artifact_format_slug, artifact_formats_for_publish_format};
pub use cmds::{
    doc::generate_for_project,
    spy::{SpyMode, SpyOptions, SpyTargetOptions, SpyTargetPlatform, run as run_spy},
};
pub use workspace::{
    AutoLinkConfig, BuildPluginSpec, BuildTargetSpec, DependencySpec, LocalLegionConfig, ProjectArtifactKind, ProjectManifest,
    PublishTargetSpec, RunnerBinding, WorkspaceDefaults, WorkspaceManifest,
};

/// On-disk layout for Legion projects (`legion.von` / `legions.von` / `legion-lock.von` / `~/.valkyrie`).
pub const LEGION_PROJECT_LAYOUT: nyar_package_manager::ProjectLayout = nyar_package_manager::ProjectLayout {
    package_manifest: "legion.von",
    workspace_manifest: "legions.von",
    ignore_file: "legion.ignore",
    lockfile: "legion-lock.von",
    home_dirname: ".valkyrie",
    home_env: "VALKYRIE_HOME",
    token_env_vars: &["VALKYRIE_TOKEN", "LEGION_TOKEN"],
    entry_aliases: no_entry_aliases,
};

/// 本地依赖覆盖配置（类似 Rust `.cargo/config.toml`），默认位于 `.config/legion/legions.von`。
pub const LOCAL_LEGION_CONFIG: &str = ".config/legion/legions.von";

fn no_entry_aliases(_: &str) -> &'static [&'static str] {
    &[]
}

pub use nyar::{
    ArtifactPolicy, CanonicalAbi, CanonicalArch, CanonicalSpecification, CanonicalTarget, CanonicalTargetParseError, CanonicalVendor,
    EntryPolicy, PublishFormat, RunnerFamily, RunnerSelector, TargetHostKind, TargetMode, TargetProfile, WrapStrategy,
};
pub use nyar_format::printer::{to_string as write_von, to_string_indented as write_von_indented};
pub use von::{VonError, VonParser, VonValue, from_str as parse_von, from_value as parse_von_value};
pub use workspace::{
    planner::{
        BuildPlan, BuildRequest, PlannedDependency, PlannedHostContract, PlannedHostProvider, PlannedProject, PlannedSemanticSourceGroup,
        WorkspaceResolver, collect_project_root_v_files, collect_project_v_files, collect_test_build_sources, collect_test_v_files,
        project_uses_single_script_layout,
    },
    script,
};
