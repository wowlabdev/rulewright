//! Extensible static analysis and executable engineering standards for Rust workspaces.

mod atomic;
mod checksum;
mod cli;
mod directory;
mod error;
mod file;
mod glob;
mod infra;
mod languages;
/// Machine-readable rule documentation for language-model consumers.
pub mod llm;
mod lock;
mod macros;
mod markdown;
/// Terminal presentation shared by the library runner and CLI.
pub mod output;
mod path;
mod registry;
/// Rule execution, reporting, fixing, and suppression cleanup.
pub mod runner;
#[cfg(test)]
mod temporary;
mod walk;
mod working_directory;

pub use cli::{run_cli, run_with_registry};
pub(crate) use glob::matches_ignore;
pub use infra::config::{Config, ConfigError};
pub use infra::{
    ctx::{FileCtx, Violation, violation},
    fix::Fix,
};
pub use languages::{
    rust::{AstCtx, RustLocation},
    toml::TomlCtx,
    workspace::{
        DependencyRecord, FunctionRecord, ShingleFingerprint, StringRecord, StructRecord,
        WorkspaceCtx, WorkspaceManifest, WorkspaceRustFile,
    },
};
pub use path::{Path, PathBuf};
pub use registry::{
    ConfigRule, ConfigValue, Example, ParamDefault, ParamType, RegistryError, Rule, RuleCheck,
    RuleFix, RuleInfo, RuleKind, RuleMeta, RulePack, RuleParam, RuleRegistry, Severity, all_rules,
};
#[cfg(test)]
pub(crate) use test_support::{
    apply_ast_fixes, apply_ast_tree_fix, apply_line_fixes, check_source, check_source_ast,
    check_source_toml, check_workspace_sources,
};

#[cfg(test)]
mod test_support;

/// Test-only third-party exports used by the rule harness macros.
#[cfg(test)]
#[doc(hidden)]
pub mod _private {
    pub use googletest::{gtest, scoped_trace, verify_false, verify_true};

    pub type TestResult<T = ()> = googletest::Result<T>;
}
