// #rw(file: rust_default_hasher) dedup set on a cold walk path; fast-hasher dependency not warranted

use std::collections::HashSet;

use crate::{
    directory::{self, EntryKind},
    path::{Path, PathBuf},
};
use gix::bstr::ByteSlice;
#[cfg(test)]
use googletest::prelude::*;

#[derive(Debug, thiserror::Error)]
#[error("{context}: {details}")]
pub(crate) struct PathDiscoveryError {
    context: Box<str>,
    details: Box<str>,
}

impl PathDiscoveryError {
    fn new(context: impl Into<Box<str>>, mut failures: Vec<String>) -> Self {
        failures.sort_unstable();
        failures.dedup();

        Self {
            context: context.into(),
            details: failures.join("; ").into_boxed_str(),
        }
    }
}

/// Get uncommitted source files with one of `extensions` via `gix`.
pub(crate) fn git_dirty_paths(
    root: &Path,
    workspace_root: &Path,
    package_filter: &[String],
    extensions: &[&str],
) -> Result<Vec<PathBuf>, PathDiscoveryError> {
    let layout = cargo_layout(workspace_root);
    let repo = gix::open(root).map_err(|error| {
        PathDiscoveryError::new(
            format!("failed to inspect dirty files under {}", root.display()),
            vec![format!("failed to open repository: {error}")],
        )
    })?;
    let platform = repo.status(gix::progress::Discard).map_err(|error| {
        PathDiscoveryError::new(
            format!("failed to inspect dirty files under {}", root.display()),
            vec![format!("failed to prepare git status: {error}")],
        )
    })?;
    let iter = platform
        .into_iter(Vec::<gix::bstr::BString>::new())
        .map_err(|error| {
            PathDiscoveryError::new(
                format!("failed to inspect dirty files under {}", root.display()),
                vec![format!("failed to iterate git status: {error}")],
            )
        })?;

    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    let mut failures = Vec::new();

    for item in iter {
        let item = match item {
            Ok(item) => item,
            Err(error) => {
                // #rw(rust_alloc_in_loop) preserve each independent traversal failure for one diagnostic
                failures.push(error.to_string());
                continue;
            }
        };
        let rel_bstr = item.location();
        let abs = root.join(rel_bstr.to_path_lossy());

        if abs
            .extension()
            .is_none_or(|extension| !extensions.iter().any(|expected| extension == *expected))
        {
            continue;
        }

        let is_file = directory::inspect(&abs)
            .ok()
            .flatten()
            .is_some_and(|entry| entry.kind() == EntryKind::File);

        if !is_file || !abs.starts_with(workspace_root) {
            continue;
        }

        if layout
            .as_ref()
            .and_then(|layout| layout.target.as_ref())
            .is_some_and(|target| abs.starts_with(target))
            || layout.as_ref().is_some_and(|layout| {
                owning_cargo_root(&abs, workspace_root)
                    .is_some_and(|owner| !layout.members.iter().any(|member| member == &owner))
            })
        {
            continue;
        }

        if !package_filter.is_empty() {
            let owner = owning_cargo_root(&abs, workspace_root);
            let selected = package_filter.iter().any(|filter| {
                if filter == "." {
                    owner
                        .as_ref()
                        .is_none_or(|owner| owner.as_path() == workspace_root)
                } else {
                    abs.starts_with(workspace_root.join(filter))
                }
            });

            if !selected {
                continue;
            }
        }

        // #rw(rust_clone_in_loop) need one copy in seen and one in paths
        if seen.insert(abs.clone()) {
            paths.push(abs);
        }
    }

    if failures.is_empty() {
        paths.sort_unstable();

        Ok(paths)
    } else {
        Err(PathDiscoveryError::new(
            format!("failed to inspect dirty files under {}", root.display()),
            failures,
        ))
    }
}

/// Walk the workspace in parallel for files with one of `extensions`.
pub(crate) fn source_paths(
    root: &Path,
    package_filter: &[String],
    extensions: &[&str],
) -> Result<Vec<PathBuf>, PathDiscoveryError> {
    let layout = cargo_layout(root);

    if package_filter.is_empty() {
        let allowed = layout
            .as_ref()
            .map_or_else(|| vec![root.to_path_buf()], |layout| layout.members.clone());

        walk_dir(
            root,
            extensions,
            &allowed,
            layout.as_ref().and_then(|layout| layout.target.as_deref()),
        )
    } else {
        let mut all = Vec::new();

        for name in package_filter {
            let crate_dir = if name == "." {
                root.to_path_buf()
            } else {
                root.join(name)
            };

            if directory::inspect(&crate_dir)
                .ok()
                .flatten()
                .is_some_and(|entry| entry.kind() == EntryKind::Directory)
            {
                all.extend(walk_dir(
                    &crate_dir,
                    extensions,
                    std::slice::from_ref(&crate_dir),
                    layout.as_ref().and_then(|layout| layout.target.as_deref()),
                )?);
            }
        }

        all.sort_unstable();
        all.dedup();

        Ok(all)
    }
}

/// Walk the workspace in parallel for `.rs` paths.
pub(crate) fn rs_paths(
    root: &Path,
    package_filter: &[String],
) -> Result<Vec<PathBuf>, PathDiscoveryError> {
    source_paths(root, package_filter, &["rs"])
}

fn walk_dir(
    root: &Path,
    extensions: &[&str],
    allowed_cargo_roots: &[PathBuf],
    target_directory: Option<&Path>,
) -> Result<Vec<PathBuf>, PathDiscoveryError> {
    let report = crate::walk::visible_source_files_with_boundaries(
        root,
        extensions,
        ".rulewrightignore",
        allowed_cargo_roots,
        target_directory,
    );
    let (paths, failures) = report.into_parts();

    if failures.is_empty() {
        Ok(paths)
    } else {
        Err(PathDiscoveryError::new(
            format!("failed to walk source tree {}", root.display()),
            failures
                .into_iter()
                .map(|failure| failure.to_string())
                .collect(),
        ))
    }
}

#[derive(Debug)]
struct CargoLayout {
    members: Vec<PathBuf>,
    target: Option<PathBuf>,
}

fn cargo_layout(root: &Path) -> Option<CargoLayout> {
    let manifest = root.join("Cargo.toml");

    if !std::path::Path::new(manifest.as_os_str()).is_file() {
        return None;
    }

    let mut command = cargo_metadata::MetadataCommand::new();

    command
        .manifest_path(std::path::PathBuf::from(manifest.as_os_str()))
        .no_deps();
    let metadata = command.exec().ok()?;
    let mut members = metadata
        .packages
        .iter()
        .filter(|package| metadata.workspace_members.contains(&package.id))
        .filter_map(|package| package.manifest_path.parent())
        .map(|path| PathBuf::from(path.as_std_path().to_path_buf()))
        .collect::<Vec<_>>();

    members.push(root.to_path_buf());
    members.sort();
    members.dedup();

    Some(CargoLayout {
        members,
        target: Some(PathBuf::from(metadata.target_directory.into_std_path_buf())),
    })
}

fn owning_cargo_root(path: &Path, workspace_root: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .take_while(|ancestor| ancestor.starts_with(workspace_root))
        .find(|ancestor| {
            let manifest = ancestor.join("Cargo.toml");

            std::path::Path::new(manifest.as_os_str()).is_file()
        })
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file;

    #[gtest]
    fn finds_rust_files() -> Result<()> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let paths = rs_paths(&directory, &[]).or_fail()?;

        verify_true!(paths.len() >= 2)?;
        verify_true!(
            paths
                .iter()
                .all(|path| path.extension().is_some_and(|extension| extension == "rs"))
        )?;

        Ok(())
    }

    #[gtest]
    fn filters_requested_languages() -> Result<()> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let paths = source_paths(&directory, &[], &["rs", "toml"]).or_fail()?;

        verify_true!(
            paths
                .iter()
                .any(|path| path.extension().is_some_and(|extension| extension == "rs"))
        )?;
        verify_true!(paths.iter().all(|path| matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("rs" | "toml")
        )))?;

        Ok(())
    }

    #[gtest]
    fn missing_source_root_is_a_fatal_walk_error() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let missing = directory.path().join("missing");
        let error = rs_paths(&missing, &[]).unwrap_err().to_string();

        verify_that!(
            error.as_str(),
            contains_substring("failed to walk source tree")
        )?;

        verify_that!(error.as_str(), contains_substring("missing"))
    }

    #[gtest]
    fn workspace_walk_includes_nested_members_and_prunes_independent_projects() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let root = directory.path();
        let member = root.join("nested/member");
        let independent = root.join("tools/independent");

        directory::ensure(&member.join("src")).or_fail()?;
        directory::ensure(&independent.join("src")).or_fail()?;
        directory::ensure(&root.join("scripts")).or_fail()?;
        file::write_text(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"nested/member\"]\nresolver = \"3\"\n",
        )
        .or_fail()?;
        file::write_text(
            &member.join("Cargo.toml"),
            "[package]\nname = \"nested-member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .or_fail()?;
        file::write_text(&member.join("src/lib.rs"), "pub fn member() {}\n").or_fail()?;
        file::write_text(&root.join("scripts/check.rs"), "pub fn check() {}\n").or_fail()?;
        file::write_text(
            &independent.join("Cargo.toml"),
            "[package]\nname = \"independent\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .or_fail()?;
        file::write_text(&independent.join("src/lib.rs"), "pub fn independent() {}\n").or_fail()?;

        let paths = source_paths(root, &[], &["rs", "toml"]).or_fail()?;

        verify_true!(paths.contains(&member.join("src/lib.rs")))?;
        verify_true!(paths.contains(&root.join("scripts/check.rs")))?;
        verify_false!(paths.contains(&independent.join("src/lib.rs")))?;

        let filtered = source_paths(root, &["nested/member".to_owned()], &["rs"]).or_fail()?;

        verify_eq!(filtered, vec![member.join("src/lib.rs")])
    }
}
