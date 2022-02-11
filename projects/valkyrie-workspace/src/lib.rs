use serde::{Deserialize, Serialize};

pub use crate::workspace::Workspace;

mod workspace;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegionConfig {
    pub workspace: Workspace,
}

pub struct ValkyrieFile {}
