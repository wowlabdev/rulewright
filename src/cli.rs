//! Command-line orchestration and Cargo workspace discovery.

use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    process::{Command, ExitCode},
};

use cargo_metadata::{Metadata, MetadataCommand};
use clap::{ArgGroup, Parser, Subcommand};
use tabled::Tabled;

use crate::{
    Config, ParamDefault, RuleParam, RuleRegistry, atomic, llm, output,
    path::{Path, PathBuf},
    runner,
};

#[derive(Debug, Parser)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Clap models independent command-line switches as booleans"
)]
#[command(
    name = "rulewright",
    version,
    about = "Executable engineering standards for Rust workspaces",
    group(
        ArgGroup::new("terminal_action")
            .args(["init", "list", "parse_config", "detail", "llm", "suppressions"])
            .multiple(false)
    )
)]
struct Args {
    /// Run a maintenance subcommand.
    #[command(subcommand)]
    command: Option<RulewrightCommand>,
    /// Run only the named rule; repeat to select more than one.
    #[arg(long, short)]
    rule: Vec<String>,
    /// Apply every available safe fix.
    #[arg(
        long,
        conflicts_with_all = ["init", "list", "parse_config", "detail", "llm", "suppressions"]
    )]
    fix: bool,
    /// Show planned changes without writing files.
    #[arg(long, global = true)]
    dry_run: bool,
    /// Run Rulewright, rustfmt, and Clippy as one local CI gate.
    #[arg(
        long,
        conflicts_with_all = ["init", "list", "parse_config", "detail", "llm", "suppressions"]
    )]
    ci: bool,
    /// Create a complete rulewright.toml without replacing an existing file.
    #[arg(long)]
    init: bool,
    /// List registered rules and their metadata.
    #[arg(long)]
    list: bool,
    /// Resolve a configuration file and print its rules as JSON.
    #[arg(long, value_name = "PATH")]
    parse_config: Option<std::path::PathBuf>,
    /// Show the justification, parameters, and examples for one rule.
    #[arg(long)]
    detail: Option<String>,
    /// Print a Markdown rule catalog and findings designed for an AI coding agent.
    #[arg(long)]
    llm: bool,
    /// Limit analysis to a Cargo package name or workspace-relative member path.
    #[arg(long, short = 'F', global = true, value_name = "SELECTOR")]
    filter: Vec<String>,
    /// Report suppression directives and their targets.
    #[arg(long)]
    suppressions: bool,
    /// Print only errors and requested machine-readable output.
    #[arg(long, short, global = true)]
    quiet: bool,
    /// Analyze only files changed according to Git.
    #[arg(long, short = 'd', global = true)]
    dirty: bool,
    /// Treat unknown or missing configuration entries as errors.
    #[arg(long)]
    strict: bool,
    /// Analyze this Cargo workspace instead of discovering one from the current directory.
    #[arg(long, global = true, value_name = "PATH")]
    workspace_root: Option<std::path::PathBuf>,
    /// Load configuration from this path instead of rulewright.toml at the workspace root.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<std::path::PathBuf>,
}

#[derive(Debug, Subcommand)]
enum RulewrightCommand {
    /// Remove suppression targets that no longer hide a violation.
    Clean,
}

#[derive(Debug)]
struct Project {
    root: PathBuf,
    metadata: Metadata,
}

/// Run the stock CLI with Rulewright's generic built-in rules.
#[must_use]
pub fn run_cli() -> ExitCode {
    let registry = match RuleRegistry::with_builtins() {
        Ok(registry) => registry,
        Err(error) => {
            output::error(&error.to_string());

            return ExitCode::FAILURE;
        }
    };

    run_with_registry(&registry)
}

/// Run the CLI with a caller-supplied registry of built-in and downstream rules.
#[must_use]
pub fn run_with_registry(registry: &RuleRegistry) -> ExitCode {
    run_args(&Args::parse(), registry)
}

fn run_args(args: &Args, registry: &RuleRegistry) -> ExitCode {
    if let Some(name) = unknown_rule_filter(registry, &args.rule) {
        output::error(&format!("unknown rule: {name}"));

        return ExitCode::FAILURE;
    }

    if !args.quiet && !args.llm && args.parse_config.is_none() {
        output::banner("Rulewright", env!("CARGO_PKG_VERSION"));
    }

    if args.list {
        print_rule_list(registry);

        return ExitCode::SUCCESS;
    }

    if let Some(config_path) = &args.parse_config {
        return print_parsed_config(registry, config_path);
    }

    if let Some(rule_name) = &args.detail {
        return print_rule_detail(registry, rule_name);
    }

    let initial_directory = match std::env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            output::error(&format!("failed to resolve the current directory: {error}"));

            return ExitCode::FAILURE;
        }
    };
    let project = match discover_project(args, &initial_directory) {
        Ok(project) => project,
        Err(error) => {
            output::error(&error);

            return ExitCode::FAILURE;
        }
    };
    let config_path = args.config.as_ref().map_or_else(
        || project.root.join("rulewright.toml"),
        |path| PathBuf::from(resolve_relative(path, &initial_directory)),
    );

    if args.init {
        return handle_init(registry, &config_path);
    }

    let config = match load_and_validate_config(registry, &config_path, args.strict) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let member_roots = match resolve_filters(&project, &args.filter) {
        Ok(filters) => filters,
        Err(error) => {
            output::error(&error);

            return ExitCode::FAILURE;
        }
    };
    let packages = workspace_packages(&project);
    let run_ctx = runner::RunCtx {
        registry,
        rule_filter: &args.rule,
        package_filter: &member_roots,
        packages: &packages,
        config: &config,
        root: &project.root,
        workspace_root: &project.root,
        quiet: args.quiet,
        dirty: args.dirty,
    };

    if matches!(&args.command, Some(RulewrightCommand::Clean)) {
        return runner::clean_suppressions(&run_ctx, args.dry_run);
    }

    if args.llm {
        return llm::print(&run_ctx);
    }

    if args.suppressions {
        return runner::report_suppressions(
            registry,
            &member_roots,
            &config,
            &project.root,
            &project.root,
            args.quiet,
        );
    }

    let fix_mode = match (args.fix, args.dry_run) {
        (true, true) => runner::FixMode::DryRun,
        (true, false) => runner::FixMode::Apply,
        (false, _) => runner::FixMode::Off,
    };
    let mut failed = !runner::run(&run_ctx, fix_mode);

    if args.ci {
        failed |= !run_cargo_check("fmt", &["fmt", "--all", "--", "--check"], &project.root);
        let clippy_target =
            runner::cargo_target_dir(&project.root, &project.root).join("rulewright-clippy");

        failed |= !run_cargo_check_with_environment(
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
            &project.root,
            &[("CARGO_TARGET_DIR", clippy_target.as_os_str())],
        );
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn unknown_rule_filter<'a>(registry: &RuleRegistry, filter: &'a [String]) -> Option<&'a str> {
    filter
        .iter()
        .find(|name| {
            !registry
                .rules()
                .iter()
                .any(|rule| rule.info.name == name.as_str())
        })
        .map(String::as_str)
}

fn discover_project(args: &Args, initial_directory: &std::path::Path) -> Result<Project, String> {
    discover_project_with_environment(args, initial_directory, std::env::var_os("RULEWRIGHT_ROOT"))
}

fn discover_project_with_environment(
    args: &Args,
    initial_directory: &std::path::Path,
    environment_root: Option<OsString>,
) -> Result<Project, String> {
    let candidate = if let Some(path) = &args.workspace_root {
        resolve_relative(path, initial_directory)
    } else if let Some(path) = environment_root {
        resolve_relative(&std::path::PathBuf::from(path), initial_directory)
    } else {
        initial_directory.to_path_buf()
    };

    if !candidate.is_dir() {
        return Err(format!(
            "workspace root candidate {} is not a directory",
            candidate.display()
        ));
    }

    let candidate = std::fs::canonicalize(&candidate).map_err(|error| {
        format!(
            "failed to canonicalize workspace root candidate {}: {error}",
            candidate.display()
        )
    })?;
    let metadata = MetadataCommand::new()
        .current_dir(&candidate)
        .no_deps()
        .exec()
        .map_err(|error| {
            format!(
                "failed to discover a Cargo root from {}: {error}",
                candidate.display()
            )
        })?;
    let root = PathBuf::from(metadata.workspace_root.clone().into_std_path_buf());

    Ok(Project { root, metadata })
}

fn resolve_filters(project: &Project, selectors: &[String]) -> Result<Vec<String>, String> {
    if selectors.is_empty() {
        return Ok(Vec::new());
    }

    let workspace_members: BTreeSet<_> = project.metadata.workspace_members.iter().collect();
    let mut selected = BTreeSet::new();

    for selector in selectors {
        if std::path::Path::new(selector).is_absolute() {
            return Err(format!(
                "package filter `{selector}` must not be an absolute path"
            ));
        }

        let normalized_selector = selector.replace('\\', "/");
        let mut matches = BTreeSet::new();

        for package in &project.metadata.packages {
            if !workspace_members.contains(&package.id) {
                continue;
            }

            let Some(member_root) = package.manifest_path.parent() else {
                continue;
            };
            let member_root = member_root.as_std_path();
            let native_root: &std::path::Path = project.root.as_ref();
            let relative = member_root.strip_prefix(native_root).map_err(|error| {
                format!(
                    "Cargo member {} is outside workspace root {}: {error}",
                    member_root.display(),
                    project.root.display()
                )
            })?;
            let relative = if relative.as_os_str().is_empty() {
                ".".to_owned()
            } else {
                relative.to_string_lossy().replace('\\', "/")
            };

            if package.name.as_str() == selector || relative == normalized_selector {
                matches.insert(relative);
            }
        }

        match matches.len() {
            0 => {
                return Err(format!(
                    "package filter `{selector}` did not match a workspace member"
                ));
            }
            1 => selected.extend(matches),
            _ => {
                return Err(format!(
                    "package filter `{selector}` is ambiguous across: {}",
                    matches.into_iter().collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }

    Ok(selected.into_iter().collect())
}

fn workspace_packages(project: &Project) -> Vec<runner::PackageRoot> {
    let workspace_members: BTreeSet<_> = project.metadata.workspace_members.iter().collect();
    let mut packages = project
        .metadata
        .packages
        .iter()
        .filter(|package| workspace_members.contains(&package.id))
        .filter_map(|package| {
            Some(runner::PackageRoot {
                name: package.name.to_string(),
                root: PathBuf::from(package.manifest_path.parent()?.as_std_path().to_path_buf()),
            })
        })
        .collect::<Vec<_>>();

    packages.sort_by(|left, right| left.root.cmp(&right.root));

    packages
}

fn resolve_relative(
    path: &std::path::Path,
    initial_directory: &std::path::Path,
) -> std::path::PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        initial_directory.join(path)
    }
}

fn rule_metadata(registry: &RuleRegistry) -> Vec<(&'static str, &'static [RuleParam])> {
    registry
        .metadata()
        .into_iter()
        .map(|rule| (rule.name, rule.params))
        .collect()
}

fn load_and_validate_config(
    registry: &RuleRegistry,
    config_path: &Path,
    strict: bool,
) -> Result<Config, ExitCode> {
    let mut config = Config::load(config_path).map_err(|error| {
        output::error(&format!("{}: {error}", config_path.display()));
        output::error("a configuration is required; run `rulewright --init`");

        ExitCode::FAILURE
    })?;
    let metadata = rule_metadata(registry);
    let (errors, warnings) = config.validate(&metadata);

    for warning in &warnings {
        output::warning(warning);
    }

    if strict && !warnings.is_empty() {
        output::error("--strict treats configuration warnings as errors");

        return Err(ExitCode::FAILURE);
    }

    if !errors.is_empty() {
        for error in errors {
            output::error(&error);
        }

        return Err(ExitCode::FAILURE);
    }

    if !warnings.is_empty() {
        config.backfill_registry_defaults(&registry.metadata());
    }

    Ok(config)
}

fn handle_init(registry: &RuleRegistry, config_path: &Path) -> ExitCode {
    let metadata = registry.metadata();
    let config = Config::generate_registry_default(&metadata);

    match atomic::create(config_path, config.to_toml_string()) {
        Ok(()) => {
            output::success(&format!(
                "created {} with {} rules enabled",
                config_path.display(),
                metadata.len()
            ));

            ExitCode::SUCCESS
        }
        Err(error) if error.is_already_exists() => {
            output::error(&format!("{} already exists", config_path.display()));

            ExitCode::FAILURE
        }
        Err(error) => {
            output::error(&format!(
                "failed to create {}: {error}",
                config_path.display()
            ));

            ExitCode::FAILURE
        }
    }
}

fn print_rule_list(registry: &RuleRegistry) {
    let rules = registry.metadata();
    let rows: Vec<RuleListRow> = rules
        .iter()
        .map(|rule| RuleListRow {
            name: rule.name.to_owned(),
            severity: rule.severity.as_str().to_owned(),
            kind: rule.kind.as_str().to_owned(),
            fixable: if rule.fixable { "yes" } else { "no" }.to_owned(),
            params: rule
                .params
                .iter()
                .map(|parameter| parameter.name)
                .collect::<Vec<_>>()
                .join(", "),
            description: rule.description.to_owned(),
        })
        .collect();
    let fixable = rows.iter().filter(|row| row.fixable == "yes").count();

    output::detail(&format!(
        "{} rules registered ({fixable} fixable)",
        rows.len()
    ));
    output::blank();
    output::table(rows);
}

#[derive(Debug, Tabled)]
struct RuleListRow {
    #[tabled(rename = "Rule")]
    name: String,
    #[tabled(rename = "Severity")]
    severity: String,
    #[tabled(rename = "Type")]
    kind: String,
    #[tabled(rename = "Fixable")]
    fixable: String,
    #[tabled(rename = "Params")]
    params: String,
    #[tabled(rename = "Description")]
    description: String,
}

fn print_parsed_config(registry: &RuleRegistry, config_path: &std::path::Path) -> ExitCode {
    let config_path = PathBuf::from(config_path.to_path_buf());
    let config = match Config::load(&config_path) {
        Ok(config) => config,
        Err(error) => {
            output::error(&error.to_string());

            return ExitCode::FAILURE;
        }
    };
    let entries = config.resolved_rules(&registry.metadata());

    match serde_json::to_string(&entries) {
        Ok(json) => {
            println!("{json}");

            ExitCode::SUCCESS
        }
        Err(error) => {
            output::error(&format!("failed to serialize configuration: {error}"));

            ExitCode::FAILURE
        }
    }
}

fn print_rule_detail(registry: &RuleRegistry, name: &str) -> ExitCode {
    let metadata = registry.metadata();
    let Some(rule) = metadata.iter().find(|rule| rule.name == name) else {
        output::error(&format!("unknown rule: {name}"));

        return ExitCode::FAILURE;
    };

    output::header(name);
    output::kv("type", rule.kind.as_str());
    output::kv("severity", rule.severity.as_str());
    output::kv("fixable", if rule.fixable { "yes" } else { "no" });
    output::kv("description", rule.description);

    for parameter in rule.params {
        let kind = match parameter.param_type {
            crate::ParamType::Int => "i64",
            crate::ParamType::StringArray => "[String]",
        };
        let default = match &parameter.default {
            ParamDefault::Int(value) => value.to_string(),
            ParamDefault::StringArray(values) => format!("{values:?}"),
        };

        output::kv(parameter.name, &format!("{kind}, default = {default}"));
    }

    output::blank();
    output::detail(rule.justification);

    for example in rule.examples {
        output::blank();

        if example.pass {
            output::success(example.label);
        } else {
            output::error(example.label);
        }

        for line in example.code.lines() {
            output::detail(line);
        }
    }

    ExitCode::SUCCESS
}

fn run_cargo_check(name: &str, arguments: &[&str], root: &Path) -> bool {
    run_cargo_check_with_environment(name, arguments, root, &[])
}

fn run_cargo_check_with_environment(
    name: &str,
    arguments: &[&str],
    root: &Path,
    environment: &[(&str, &OsStr)],
) -> bool {
    output::header(&format!("cargo {name}"));
    let mut command = Command::new("cargo");

    for (name, _) in std::env::vars_os() {
        if is_cargo_package_context(&name) {
            command.env_remove(name);
        }
    }

    let native_root: &std::path::Path = root.as_ref();
    let status = command
        .args(arguments)
        .current_dir(native_root)
        .envs(environment.iter().copied())
        .status();

    match status {
        Ok(status) if status.success() => {
            output::success(&format!("cargo {name} passed"));

            true
        }
        Ok(_) => {
            output::error(&format!("cargo {name} failed"));

            false
        }
        Err(error) => {
            output::error(&format!("cargo {name}: {error}"));

            false
        }
    }
}

fn is_cargo_package_context(name: &OsString) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };

    name.starts_with("CARGO_PKG_")
        || matches!(
            name,
            "CARGO_BIN_NAME"
                | "CARGO_CRATE_NAME"
                | "CARGO_MANIFEST_DIR"
                | "CARGO_MANIFEST_PATH"
                | "CARGO_PRIMARY_PACKAGE"
        )
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory as _;

    use super::*;

    fn args(values: &[&str]) -> Args {
        Args::try_parse_from(std::iter::once("rulewright").chain(values.iter().copied()))
            .expect("test arguments should parse")
    }

    #[test]
    fn long_help_explains_every_top_level_option() {
        let help = Args::command().render_long_help().to_string();

        for explanation in [
            "Run only the named rule",
            "Apply every available safe fix",
            "Show planned changes",
            "one local CI gate",
            "complete rulewright.toml",
            "List registered rules",
            "print its rules as JSON",
            "justification, parameters, and examples",
            "designed for an AI coding agent",
            "Cargo package name",
            "Report suppression directives",
            "Print only errors",
            "changed according to Git",
            "configuration entries as errors",
            "Analyze this Cargo workspace",
            "instead of rulewright.toml",
        ] {
            assert!(
                help.contains(explanation),
                "missing help text: {explanation}"
            );
        }
    }

    #[test]
    fn terminal_actions_reject_ambiguous_combinations() {
        for arguments in [
            ["rulewright", "--ci", "--llm"].as_slice(),
            ["rulewright", "--fix", "--llm"].as_slice(),
            ["rulewright", "--list", "--init"].as_slice(),
        ] {
            let error = Args::try_parse_from(arguments).expect_err("actions should conflict");

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

    fn root_package(root: &Path) {
        crate::directory::ensure(&root.join("src")).expect("source directory should be created");
        crate::file::write_text(
            &root.join("Cargo.toml"),
            "[package]\nname = \"root-package\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("root manifest should be written");
        crate::file::write_text(&root.join("src/lib.rs"), "pub fn root() {}\n")
            .expect("root source should be written");
    }

    fn canonical_path(path: &Path) -> std::path::PathBuf {
        std::fs::canonicalize(path).expect("test path should be canonicalizable")
    }

    #[test]
    fn discovery_ascends_from_a_root_package_descendant() {
        let temporary = crate::temporary::Directory::new().expect("temporary directory");
        let root = temporary.path();

        root_package(root);
        let descendant = root.join("src");
        let project = discover_project_with_environment(
            &args(&[]),
            std::path::Path::new(descendant.as_os_str()),
            None,
        )
        .expect("root package should be discovered");

        assert_eq!(canonical_path(project.root.as_path()), canonical_path(root));
    }

    #[test]
    fn explicit_root_wins_and_nested_members_resolve_by_name_or_path() {
        let temporary = crate::temporary::Directory::new().expect("temporary directory");
        let selected = temporary.path().join("selected");
        let ignored = temporary.path().join("ignored");
        let member = selected.join("nested/member");

        crate::directory::ensure(&member.join("src")).expect("member source directory");
        crate::directory::ensure(&ignored).expect("ignored directory");
        crate::file::write_text(
            &selected.join("Cargo.toml"),
            "[workspace]\nmembers = [\"nested/member\"]\nresolver = \"3\"\n",
        )
        .expect("workspace manifest");
        crate::file::write_text(
            &member.join("Cargo.toml"),
            "[package]\nname = \"nested-member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("member manifest");
        crate::file::write_text(&member.join("src/lib.rs"), "pub fn member() {}\n")
            .expect("member source");

        let selected_arg = selected.to_string_lossy();
        let parsed = args(&["--workspace-root", &selected_arg]);
        let project = discover_project_with_environment(
            &parsed,
            std::path::Path::new(ignored.as_os_str()),
            Some(ignored.as_os_str().to_owned()),
        )
        .expect("explicit workspace should win");

        assert_eq!(
            canonical_path(project.root.as_path()),
            canonical_path(&selected)
        );
        assert_eq!(
            resolve_filters(&project, &["nested-member".to_owned()]).expect("package filter"),
            vec!["nested/member"]
        );
        assert_eq!(
            resolve_filters(&project, &["nested/member".to_owned()]).expect("path filter"),
            vec!["nested/member"]
        );
        assert!(resolve_filters(&project, &["missing".to_owned()]).is_err());
        assert!(resolve_filters(&project, &[selected_arg.into_owned()]).is_err());
    }
}
