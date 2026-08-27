//! Persistent content-addressed analysis cache.

use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

use crate::{
    atomic,
    checksum::{self, Checksum, TreeSnapshot},
    directory, file,
    lock::Lock,
    path::{Path, PathBuf},
};
use serde::{Deserialize, Serialize};

use super::{FileResult, RunCtx};
use crate::{
    Rule, Violation,
    languages::{
        Analysis,
        workspace::{WorkspaceCtx, WorkspaceManifest, WorkspaceRustFile},
    },
};

const CACHE_SCHEMA: u32 = 1;
const BASE_DOMAIN: &[u8] = b"rulewright:analysis-base:v1";
const RUST_CONTEXT_DOMAIN: &[u8] = b"rulewright:rust-context:v1";
const TOML_CONTEXT_DOMAIN: &[u8] = b"rulewright:toml-context:v1";
const WORKSPACE_RULE_DOMAIN: &[u8] = b"rulewright:workspace-rule:v1";

pub(super) struct Session {
    _lock: Lock,
    root: PathBuf,
    cache_path: PathBuf,
    fast_path: PathBuf,
    workspace_path: PathBuf,
    base: Checksum,
    tree_checksum: Checksum,
    rust_context: Checksum,
    toml_context: Checksum,
    file_checksums: BTreeMap<PathBuf, Checksum>,
    cached: BTreeMap<String, CachedAnalysis>,
    complete: Option<Vec<CachedViolation>>,
    workspace: Mutex<BTreeMap<String, CachedWorkspaceRule>>,
}

impl Session {
    pub(super) fn open(ctx: &RunCtx<'_>, rules: &[&'static Rule]) -> Option<Self> {
        let source_paths =
            crate::infra::walk::source_paths(ctx.workspace_root, &[], &["rs", "toml"]).ok()?;
        let snapshot = TreeSnapshot::capture(ctx.workspace_root, &source_paths).ok()?;
        let base = base_checksum(ctx, rules)?;
        let rust_context = rust_context_checksum(&snapshot)?;
        let toml_context = toml_context_checksum(ctx.root, &snapshot)?;
        let file_checksums = snapshot
            .entries()
            .iter()
            .map(|entry| {
                (
                    ctx.workspace_root.join(entry.relative_path()),
                    entry.checksum(),
                )
            })
            .collect();
        let cache_dir =
            super::cargo_target_dir(ctx.root, ctx.workspace_root).join("rulewright-cache");

        directory::ensure(&cache_dir).ok()?;

        let lock_path = cache_dir.join("analysis.lock");
        let cache_path = cache_dir.join("analysis.bin");
        let fast_path = cache_dir.join("complete.bin");
        let workspace_path = cache_dir.join("workspace.bin");
        let lock = Lock::try_acquire(&lock_path).ok()?;
        let tree_checksum = snapshot.checksum();
        let complete = read_binary::<CompleteDocument>(&fast_path)
            .filter(|document| {
                document.schema == CACHE_SCHEMA
                    && document.base == base
                    && document.tree_checksum == tree_checksum
            })
            .map(|document| document.violations);
        let cached = if complete.is_some() {
            BTreeMap::new()
        } else {
            read_binary::<CacheDocument>(&cache_path)
                .filter(|document| document.schema == CACHE_SCHEMA && document.base == base)
                .map_or_else(BTreeMap::new, |document| document.files)
        };
        let workspace = if complete.is_some() {
            BTreeMap::new()
        } else {
            read_binary::<WorkspaceDocument>(&workspace_path)
                .filter(|document| document.schema == CACHE_SCHEMA && document.base == base)
                .map_or_else(BTreeMap::new, |document| document.rules)
        };

        Some(Self {
            _lock: lock,
            root: ctx.root.to_path_buf(),
            cache_path,
            fast_path,
            workspace_path,
            base,
            tree_checksum,
            rust_context,
            toml_context,
            file_checksums,
            cached,
            complete,
            workspace: Mutex::new(workspace),
        })
    }

    pub(super) fn complete(&self, rules: &[&'static Rule]) -> Option<Vec<Violation>> {
        self.complete
            .as_ref()?
            .iter()
            .map(|violation| violation.restore(rules))
            .collect()
    }

    pub(super) fn restore(&self, path: &Path, rules: &[&'static Rule]) -> Option<FileResult> {
        let rel = relative_string(path, &self.root)?;
        let cached = self.cached.get(&rel)?;
        let checksum = *self.file_checksums.get(path)?;
        let context = self.context_for(path)?;

        if cached.file_checksum != checksum || cached.context != context {
            return None;
        }

        let violations = cached
            .violations
            .iter()
            .map(|violation| violation.restore(rules))
            .collect::<Option<Vec<_>>>()?;

        Some(FileResult {
            path: path.to_path_buf(),
            checksum,
            analysis: Analysis {
                violations,
                fixes: Vec::new(),
                tree_fixes: Vec::new(),
                workspace_files: cached.workspace_files.clone(),
                workspace_manifests: cached.workspace_manifests.clone(),
            },
        })
    }

    pub(super) fn persist(&self, root: &Path, results: &[FileResult]) {
        let files = results
            .iter()
            .filter_map(|result| {
                let rel = relative_string(&result.path, root)?;
                let file_checksum = *self.file_checksums.get(&result.path)?;
                let context = self.context_for(&result.path)?;

                Some((
                    rel,
                    CachedAnalysis {
                        file_checksum,
                        context,
                        violations: result
                            .analysis
                            .violations
                            .iter()
                            .map(CachedViolation::capture)
                            .collect(),
                        workspace_files: result.analysis.workspace_files.clone(),
                        workspace_manifests: result.analysis.workspace_manifests.clone(),
                    },
                ))
            })
            .collect();
        let document = CacheDocument {
            schema: CACHE_SCHEMA,
            base: self.base,
            files,
        };
        let Ok(contents) = bincode::serde::encode_to_vec(&document, bincode::config::standard())
        else {
            return;
        };

        let _ = atomic::replace(&self.cache_path, contents);
    }

    pub(super) fn persist_complete(&self, violations: &[Violation]) {
        let document = CompleteDocument {
            schema: CACHE_SCHEMA,
            base: self.base,
            tree_checksum: self.tree_checksum,
            violations: violations.iter().map(CachedViolation::capture).collect(),
        };
        let Ok(contents) = bincode::serde::encode_to_vec(&document, bincode::config::standard())
        else {
            return;
        };

        let _ = atomic::replace(&self.fast_path, contents);
    }

    pub(super) fn workspace_violations(
        &self,
        rule: &'static Rule,
        ctx: &WorkspaceCtx<'_>,
        compute: impl FnOnce() -> Vec<Violation>,
    ) -> Vec<Violation> {
        let Some(input) = workspace_rule_checksum(rule.info.name, ctx) else {
            return compute();
        };
        let restored = {
            let workspace = self.workspace();

            workspace
                .get(rule.info.name)
                .filter(|cached| cached.input == input)
                .and_then(|cached| {
                    cached
                        .violations
                        .iter()
                        .map(|violation| violation.restore(&[rule]))
                        .collect::<Option<Vec<_>>>()
                })
        };

        if let Some(violations) = restored {
            return violations;
        }

        let violations = compute();

        self.workspace().insert(
            rule.info.name.to_owned(),
            CachedWorkspaceRule {
                input,
                violations: violations.iter().map(CachedViolation::capture).collect(),
            },
        );

        violations
    }

    pub(super) fn persist_workspace(&self) {
        let document = WorkspaceDocument {
            schema: CACHE_SCHEMA,
            base: self.base,
            rules: self.workspace().clone(),
        };
        let Ok(contents) = bincode::serde::encode_to_vec(&document, bincode::config::standard())
        else {
            return;
        };

        let _ = atomic::replace(&self.workspace_path, contents);
    }

    fn workspace(&self) -> MutexGuard<'_, BTreeMap<String, CachedWorkspaceRule>> {
        self.workspace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn context_for(&self, path: &Path) -> Option<Checksum> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("rs") => Some(self.rust_context),
            Some("toml") => Some(self.toml_context),
            _ => None,
        }
    }
}

fn read_binary<T>(path: &Path) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    let contents = match file::read_bytes(path) {
        Ok(contents) => contents,
        Err(error) if error.is_not_found() => return None,
        Err(_) => return None,
    };
    let (document, consumed) =
        bincode::serde::decode_from_slice(&contents, bincode::config::standard()).ok()?;

    (consumed == contents.len()).then_some(document)
}

#[derive(Deserialize, Serialize)]
struct CacheDocument {
    schema: u32,
    base: Checksum,
    files: BTreeMap<String, CachedAnalysis>,
}

#[derive(Deserialize, Serialize)]
struct CompleteDocument {
    schema: u32,
    base: Checksum,
    tree_checksum: Checksum,
    violations: Vec<CachedViolation>,
}

#[derive(Deserialize, Serialize)]
struct WorkspaceDocument {
    schema: u32,
    base: Checksum,
    rules: BTreeMap<String, CachedWorkspaceRule>,
}

#[derive(Clone, Deserialize, Serialize)]
struct CachedWorkspaceRule {
    input: Checksum,
    violations: Vec<CachedViolation>,
}

#[derive(Deserialize, Serialize)]
struct CachedAnalysis {
    file_checksum: Checksum,
    context: Checksum,
    violations: Vec<CachedViolation>,
    workspace_files: Vec<WorkspaceRustFile>,
    workspace_manifests: Vec<WorkspaceManifest>,
}

#[derive(Clone, Deserialize, Serialize)]
struct CachedViolation {
    rel: String,
    line: usize,
    message: String,
    rule: Option<String>,
}

impl CachedViolation {
    fn capture(violation: &Violation) -> Self {
        Self {
            rel: violation.rel.clone(),
            line: violation.line,
            message: violation.message.clone(),
            rule: violation.rule.map(str::to_owned),
        }
    }

    fn restore(&self, rules: &[&'static Rule]) -> Option<Violation> {
        let rule = self.rule.as_deref().map_or(Some(None), |name| {
            rules
                .iter()
                .find(|rule| rule.info.name == name)
                .map(|rule| Some(rule.info.name))
        })?;

        Some(Violation {
            rel: self.rel.clone(),
            line: self.line,
            message: self.message.clone(),
            rule,
        })
    }
}

fn base_checksum(ctx: &RunCtx<'_>, rules: &[&'static Rule]) -> Option<Checksum> {
    let executable = checksum::current_executable().ok()?;
    let config = serde_json::to_vec(ctx.config).ok()?;
    let mut encoded = Vec::new();

    append_part(&mut encoded, BASE_DOMAIN)?;
    append_part(&mut encoded, env!("CARGO_PKG_VERSION").as_bytes())?;
    append_part(&mut encoded, executable.as_bytes())?;
    append_part(&mut encoded, &config)?;

    for pack in ctx.registry.packs() {
        append_part(&mut encoded, pack.name.as_bytes())?;
        append_part(&mut encoded, pack.version.as_bytes())?;
        append_part(&mut encoded, pack.implementation_fingerprint.as_bytes())?;
    }

    for rule in rules {
        append_part(&mut encoded, rule.info.name.as_bytes())?;
        append_part(&mut encoded, rule.info.description.as_bytes())?;
        append_part(&mut encoded, rule.info.justification.as_bytes())?;
        append_part(&mut encoded, rule.info.severity.as_str().as_bytes())?;
        append_part(&mut encoded, rule.check.kind().as_str().as_bytes())?;
        append_part(&mut encoded, &[u8::from(rule.fix.is_some())])?;

        for parameter in rule.info.params {
            append_part(&mut encoded, parameter.name.as_bytes())?;
            append_part(&mut encoded, parameter.param_type.as_str().as_bytes())?;

            match &parameter.default {
                crate::ParamDefault::Int(value) => {
                    append_part(&mut encoded, &value.to_le_bytes())?;
                }
                crate::ParamDefault::StringArray(values) => {
                    for value in *values {
                        append_part(&mut encoded, value.as_bytes())?;
                    }
                }
            }
        }
    }

    Some(checksum::bytes(encoded))
}

fn workspace_rule_checksum(name: &str, ctx: &WorkspaceCtx<'_>) -> Option<Checksum> {
    let material = match name {
        "rust_duplicate_strings" => bincode::serde::encode_to_vec(
            ctx.files
                .iter()
                .map(|file| (&file.rel, &file.strings, &file.suppressions))
                .collect::<Vec<_>>(),
            bincode::config::standard(),
        )
        .ok()?,
        "rust_param_clump" => bincode::serde::encode_to_vec(
            ctx.files
                .iter()
                .map(|file| {
                    (
                        &file.rel,
                        file.functions
                            .iter()
                            .map(|function| {
                                (
                                    &function.name,
                                    function.line,
                                    &function.params,
                                    &function.pass_through_calls,
                                )
                            })
                            .collect::<Vec<_>>(),
                        &file.suppressions,
                    )
                })
                .collect::<Vec<_>>(),
            bincode::config::standard(),
        )
        .ok()?,
        "rust_similar_fns" => bincode::serde::encode_to_vec(
            ctx.files
                .iter()
                .map(|file| {
                    (
                        &file.rel,
                        file.functions
                            .iter()
                            .map(|function| {
                                (
                                    &function.name,
                                    function.line,
                                    function.body_token_count,
                                    function.body_checksum,
                                    &function.body_shingles,
                                )
                            })
                            .collect::<Vec<_>>(),
                        &file.suppressions,
                    )
                })
                .collect::<Vec<_>>(),
            bincode::config::standard(),
        )
        .ok()?,
        "rust_similar_structs" => bincode::serde::encode_to_vec(
            ctx.files
                .iter()
                .map(|file| (&file.rel, &file.structs, &file.suppressions))
                .collect::<Vec<_>>(),
            bincode::config::standard(),
        )
        .ok()?,
        "toml_cargo_unused_deps" => bincode::serde::encode_to_vec(
            (
                ctx.files
                    .iter()
                    .map(|file| (&file.rel, &file.crate_roots))
                    .collect::<Vec<_>>(),
                ctx.manifests,
            ),
            bincode::config::standard(),
        )
        .ok()?,
        _ => return None,
    };
    let mut encoded = Vec::new();

    append_part(&mut encoded, WORKSPACE_RULE_DOMAIN)?;
    append_part(&mut encoded, name.as_bytes())?;
    append_part(&mut encoded, &material)?;

    Some(checksum::bytes(encoded))
}

fn rust_context_checksum(snapshot: &TreeSnapshot) -> Option<Checksum> {
    let mut encoded = Vec::new();

    append_part(&mut encoded, RUST_CONTEXT_DOMAIN)?;

    for entry in snapshot.entries() {
        let path = entry.relative_path();
        let cargo_manifest = path.file_name().is_some_and(|name| name == "Cargo.toml");

        if !cargo_manifest {
            continue;
        }

        append_part(&mut encoded, path.to_str()?.as_bytes())?;
        append_part(&mut encoded, entry.checksum().as_bytes())?;
    }

    Some(checksum::bytes(encoded))
}

fn toml_context_checksum(root: &Path, snapshot: &TreeSnapshot) -> Option<Checksum> {
    let mut encoded = Vec::new();

    append_part(&mut encoded, TOML_CONTEXT_DOMAIN)?;
    append_part(&mut encoded, snapshot.checksum().as_bytes())?;

    let cargo_config = root.join(".cargo/config.toml");

    match directory::inspect_link(&cargo_config).ok()? {
        Some(_) => append_part(&mut encoded, checksum::file(&cargo_config).ok()?.as_bytes())?,
        None => append_part(&mut encoded, b"missing")?,
    }

    Some(checksum::bytes(encoded))
}

fn append_part(output: &mut Vec<u8>, value: &[u8]) -> Option<()> {
    let len = u64::try_from(value.len()).ok()?;

    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(value);

    Some(())
}

fn relative_string(path: &Path, root: &Path) -> Option<String> {
    path.strip_prefix(root).ok()?.to_slash()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::temporary;
    use googletest::prelude::*;
    use std::collections::HashSet;

    use super::*;
    use crate::{
        Config,
        infra::ignore::Suppressions,
        languages::workspace::{FunctionRecord, ShingleFingerprint},
    };

    #[gtest]
    fn binary_cache_rejects_trailing_or_corrupt_data() -> Result<()> {
        let temporary = temporary::Directory::new().or_fail()?;
        let path = temporary.path().join("cache.bin");
        let document = CompleteDocument {
            schema: CACHE_SCHEMA,
            base: checksum::bytes(b"base"),
            tree_checksum: checksum::bytes(b"tree"),
            violations: Vec::new(),
        };
        let mut contents =
            bincode::serde::encode_to_vec(&document, bincode::config::standard()).or_fail()?;

        file::write_bytes(&path, &contents).or_fail()?;
        verify_that!(
            read_binary::<CompleteDocument>(&path).map(|decoded| decoded.schema),
            some(eq(CACHE_SCHEMA))
        )?;

        contents.push(0);
        file::write_bytes(&path, &contents).or_fail()?;
        verify_that!(read_binary::<CompleteDocument>(&path).is_none(), is_true())?;

        file::write_bytes(&path, b"not a rulewright cache").or_fail()?;
        verify_that!(read_binary::<CompleteDocument>(&path).is_none(), is_true())?;

        Ok(())
    }

    #[gtest]
    fn rust_context_tracks_manifests_but_not_individual_sources() -> Result<()> {
        let temporary = temporary::Directory::new().or_fail()?;
        let workspace_root = temporary.path();
        let manifest = workspace_root.join("example/Cargo.toml");
        let source = workspace_root.join("example/src/lib.rs");

        directory::ensure(source.parent().or_fail()?).or_fail()?;
        file::write_text(&manifest, "[package]\nname = \"example\"\n").or_fail()?;
        file::write_text(&source, "pub fn value() -> u8 { 1 }\n").or_fail()?;

        let initial_snapshot =
            TreeSnapshot::capture(workspace_root, [&manifest, &source]).or_fail()?;

        let initial = rust_context_checksum(&initial_snapshot).or_fail()?;

        file::write_text(&source, "pub fn value() -> u8 { 2 }\n").or_fail()?;
        let source_snapshot =
            TreeSnapshot::capture(workspace_root, [&manifest, &source]).or_fail()?;

        verify_that!(rust_context_checksum(&source_snapshot), some(eq(initial)))?;

        file::write_text(
            &manifest,
            "[package]\nname = \"example\"\nversion = \"1.0.0\"\n",
        )
        .or_fail()?;
        let manifest_snapshot =
            TreeSnapshot::capture(workspace_root, [&manifest, &source]).or_fail()?;

        verify_that!(
            rust_context_checksum(&manifest_snapshot),
            some(not(eq(initial)))
        )?;

        Ok(())
    }

    #[gtest]
    fn workspace_rule_keys_track_only_the_records_each_rule_reads() -> Result<()> {
        let config = Config::generate_default(&[]);
        let mut files = vec![WorkspaceRustFile {
            rel: "crates/example/src/lib.rs".to_owned(),
            structs: Vec::new(),
            functions: vec![FunctionRecord {
                name: "run".to_owned(),
                line: 1,
                body_token_count: 42,
                body_checksum: checksum::bytes(b"body-one"),
                body_shingles: Box::<[ShingleFingerprint]>::default(),
                params: vec![("value".to_owned(), "u32".to_owned())],
                pass_through_calls: Box::default(),
            }],
            strings: Vec::new(),
            crate_roots: HashSet::default(),
            suppressions: Suppressions {
                lines: HashMap::default(),
                file_rules: Vec::new(),
                file_all: false,
                entries: Vec::new(),
            },
        }];
        let ctx = WorkspaceCtx {
            files: &files,
            manifests: &[],
            config: &config,
        };
        let initial_functions = workspace_rule_checksum("rust_similar_fns", &ctx).or_fail()?;
        let initial_params = workspace_rule_checksum("rust_param_clump", &ctx).or_fail()?;

        files[0].functions[0].body_checksum = checksum::bytes(b"body-two");
        let ctx = WorkspaceCtx {
            files: &files,
            manifests: &[],
            config: &config,
        };
        let body_functions = workspace_rule_checksum("rust_similar_fns", &ctx).or_fail()?;
        let body_params = workspace_rule_checksum("rust_param_clump", &ctx).or_fail()?;

        verify_that!(body_functions, not(eq(initial_functions)))?;
        verify_that!(body_params, eq(initial_params))?;

        files[0].functions[0]
            .params
            .push(("other".to_owned(), "bool".to_owned()));
        let ctx = WorkspaceCtx {
            files: &files,
            manifests: &[],
            config: &config,
        };

        verify_that!(
            workspace_rule_checksum("rust_similar_fns", &ctx),
            some(eq(body_functions))
        )?;

        verify_that!(
            workspace_rule_checksum("rust_param_clump", &ctx),
            some(not(eq(body_params)))
        )
    }
}
