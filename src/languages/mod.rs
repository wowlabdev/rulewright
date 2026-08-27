//! Language adapters for parsing source files and executing typed rules.

pub(crate) mod rust;
pub(crate) mod toml;
pub(crate) mod unicode;
pub(crate) mod workspace;

use workspace::{WorkspaceManifest, WorkspaceRustFile};

use crate::{Fix, Violation, infra::fix::TreeFix};

#[derive(Debug, Default)]
pub(crate) struct Analysis {
    pub violations: Vec<Violation>,
    pub fixes: Vec<(String, Fix)>,
    pub tree_fixes: Vec<TreeFix>,
    pub workspace_files: Vec<WorkspaceRustFile>,
    pub workspace_manifests: Vec<WorkspaceManifest>,
}
