//! Reviewed recursive repository traversal.

use crate::path::{Path, PathBuf};

/// Failure while traversing a repository tree.
#[derive(Debug, thiserror::Error)]
#[error("failed to walk repository {}: {source}", root.display())]
pub(crate) struct Error {
    root: PathBuf,
    #[source]
    source: ignore::Error,
}

/// Completed workspace-source traversal, including every entry failure.
#[derive(Debug)]
pub(crate) struct WalkReport {
    files: Vec<PathBuf>,
    failures: Vec<Error>,
}

impl WalkReport {
    /// Return discovered regular files in stable path order.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Return every failure observed while continuing the traversal.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn failures(&self) -> &[Error] {
        &self.failures
    }

    /// Consume the report into its discovered files and failures.
    #[must_use]
    pub(crate) fn into_parts(self) -> (Vec<PathBuf>, Vec<Error>) {
        (self.files, self.failures)
    }
}

/// Discover regular files below a repository root in stable path order.
///
/// Hidden and Git-ignored entries are omitted while local and parent ignore files are honored without consulting user-specific global excludes.
///
/// # Errors
///
/// Returns an error when the root or any traversed entry cannot be read.
#[cfg(test)]
pub(crate) fn repository_files(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut builder = ignore::WalkBuilder::new(root);

    builder
        .hidden(true)
        .parents(true)
        .git_ignore(true)
        .require_git(false)
        .git_global(false)
        .git_exclude(false);

    let mut paths = Vec::new();

    for entry in builder.build() {
        let entry = entry.map_err(|source| Error {
            root: root.to_path_buf(),
            source,
        })?;

        if entry.file_type().is_some_and(|kind| kind.is_file()) {
            paths.push(PathBuf::from(entry.into_path()));
        }
    }

    paths.sort();

    Ok(paths)
}

/// Discover the complete source-file view used by workspace tooling.
///
/// Hidden source files are included, Git-ignored files are omitted, and the repository's `.git` directory is pruned.
/// Local and parent ignore files are honored without consulting user-specific global excludes.
///
/// Unlike [`repository_files`], traversal continues after entry failures and returns every failure alongside the discovered regular files.
#[must_use]
#[cfg(test)]
pub(crate) fn workspace_source_files(root: &Path) -> WalkReport {
    let git_directory = root.join(".git");
    let mut builder = ignore::WalkBuilder::new(root);

    builder
        .hidden(false)
        .parents(true)
        .git_ignore(true)
        .require_git(false)
        .git_global(false)
        .git_exclude(false)
        .filter_entry(move |entry| !entry.path().starts_with(&git_directory));

    let mut files = Vec::new();
    let mut failures = Vec::new();

    for entry in builder.build() {
        match entry {
            Ok(entry) if entry.file_type().is_some_and(|kind| kind.is_file()) => {
                files.push(PathBuf::from(entry.into_path()));
            }
            Ok(_) => {}
            Err(source) => failures.push(Error {
                root: root.to_path_buf(),
                source,
            }),
        }
    }

    files.sort();
    failures.sort_by_cached_key(ToString::to_string);

    WalkReport { files, failures }
}

/// Discover visible source files selected by extension.
///
/// Hidden, Git-ignored, and project-ignored entries are omitted.
/// Local and parent ignore files are honored without consulting user-specific excludes.
///
/// Traversal continues after entry failures and returns files and failures together in stable order.
#[must_use]
#[cfg(test)]
pub(crate) fn visible_source_files(
    root: &Path,
    extensions: &[&str],
    project_ignore_filename: &str,
) -> WalkReport {
    visible_source_files_with_boundaries(root, extensions, project_ignore_filename, &[], None)
}

/// Discover visible source files while pruning Cargo projects outside an allowed workspace.
#[must_use]
pub(crate) fn visible_source_files_with_boundaries(
    root: &Path,
    extensions: &[&str],
    project_ignore_filename: &str,
    allowed_cargo_roots: &[PathBuf],
    target_directory: Option<&Path>,
) -> WalkReport {
    let mut builder = ignore::WalkBuilder::new(root);
    let root_path = root.to_path_buf();
    let allowed = allowed_cargo_roots.to_vec();
    let target = target_directory.map(Path::to_path_buf);

    builder
        .hidden(true)
        .parents(true)
        .git_ignore(true)
        .require_git(false)
        .git_global(false)
        .git_exclude(false)
        .add_custom_ignore_filename(project_ignore_filename)
        .filter_entry(move |entry| {
            let path = Path::new(entry.path());

            if target
                .as_ref()
                .is_some_and(|target| path.starts_with(target))
            {
                return false;
            }

            if path == root_path.as_path() || !entry.file_type().is_some_and(|kind| kind.is_dir()) {
                return true;
            }

            let manifest = path.join("Cargo.toml");
            let is_cargo_root = std::path::Path::new(manifest.as_os_str()).is_file();

            !is_cargo_root || allowed.iter().any(|allowed| allowed.as_path() == path)
        });

    let mut files = Vec::new();
    let mut failures = Vec::new();

    for entry in builder.build() {
        match entry {
            Ok(entry)
                if entry.file_type().is_some_and(|kind| kind.is_file())
                    && entry.path().extension().is_some_and(|extension| {
                        extensions.iter().any(|expected| extension == *expected)
                    }) =>
            {
                files.push(PathBuf::from(entry.into_path()));
            }
            Ok(_) => {}
            Err(source) => failures.push(Error {
                root: root.to_path_buf(),
                source,
            }),
        }
    }

    files.sort();
    files.dedup();
    failures.sort_by_cached_key(ToString::to_string);

    WalkReport { files, failures }
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::{repository_files, visible_source_files, workspace_source_files};
    use crate::{directory, file, temporary::Directory};

    #[gtest]
    fn repository_walk_is_sorted_and_honors_gitignore() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let root = directory.path();

        directory::ensure(&root.join("nested")).or_fail()?;
        file::write_text(&root.join(".gitignore"), "ignored.toml\n").or_fail()?;
        file::write_text(&root.join("nested/b.toml"), "").or_fail()?;
        file::write_text(&root.join("a.toml"), "").or_fail()?;
        file::write_text(&root.join("ignored.toml"), "").or_fail()?;

        verify_eq!(
            repository_files(root).or_fail()?,
            vec![root.join("a.toml"), root.join("nested/b.toml")]
        )?;

        Ok(())
    }

    #[gtest]
    fn workspace_walk_has_stable_source_policy() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let root = directory.path();

        directory::ensure(&root.join(".git/objects")).or_fail()?;
        directory::ensure(&root.join(".hidden")).or_fail()?;
        file::write_text(&root.join(".gitignore"), "ignored.rs\n").or_fail()?;
        file::write_text(&root.join(".git/objects/internal"), "").or_fail()?;
        file::write_text(&root.join(".hidden/source.rs"), "").or_fail()?;
        file::write_text(&root.join("z.rs"), "").or_fail()?;
        file::write_text(&root.join("a.rs"), "").or_fail()?;
        file::write_text(&root.join("ignored.rs"), "").or_fail()?;

        let report = workspace_source_files(root);

        verify_that!(report.failures(), is_empty())?;
        verify_eq!(
            report.files(),
            &[
                root.join(".gitignore"),
                root.join(".hidden/source.rs"),
                root.join("a.rs"),
                root.join("z.rs"),
            ]
        )?;

        Ok(())
    }

    #[gtest]
    fn workspace_walk_exposes_entry_failures() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let missing = directory.path().join("missing");
        let report = workspace_source_files(&missing);
        let missing = missing.display().to_string();

        verify_that!(report.files(), is_empty())?;
        verify_that!(report.failures(), not(is_empty()))?;
        verify_that!(
            report.failures()[0].to_string().as_str(),
            contains_substring(missing.as_str())
        )?;

        Ok(())
    }

    #[gtest]
    fn visible_source_walk_applies_project_policy() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let root = directory.path();

        directory::ensure(&root.join(".hidden")).or_fail()?;
        directory::ensure(&root.join("directory.rs")).or_fail()?;
        directory::ensure(&root.join("nested")).or_fail()?;
        file::write_text(&root.join(".gitignore"), "git-ignored.rs\n").or_fail()?;
        file::write_text(&root.join(".rulewrightignore"), "rulewright-ignored.rs\n").or_fail()?;
        file::write_text(&root.join(".hidden/source.rs"), "").or_fail()?;
        file::write_text(&root.join("git-ignored.rs"), "").or_fail()?;
        file::write_text(&root.join("rulewright-ignored.rs"), "").or_fail()?;
        file::write_text(&root.join("nested/z.rs"), "").or_fail()?;
        file::write_text(&root.join("a.rs"), "").or_fail()?;
        file::write_text(&root.join("source.toml"), "").or_fail()?;

        let report = visible_source_files(root, &["rs"], ".rulewrightignore");

        verify_that!(report.failures(), is_empty())?;
        verify_eq!(
            report.files(),
            &[root.join("a.rs"), root.join("nested/z.rs")]
        )?;

        Ok(())
    }

    #[gtest]
    fn visible_source_walk_honors_parent_ignore_files() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let root = directory.path();
        let child = root.join("child");

        directory::ensure(&child).or_fail()?;
        file::write_text(&root.join(".gitignore"), "parent-ignored.rs\n").or_fail()?;
        file::write_text(&child.join("parent-ignored.rs"), "").or_fail()?;
        file::write_text(&child.join("source.rs"), "").or_fail()?;

        let report = visible_source_files(&child, &["rs"], ".rulewrightignore");

        verify_that!(report.failures(), is_empty())?;
        verify_eq!(report.files(), &[child.join("source.rs")])?;

        Ok(())
    }

    #[gtest]
    fn visible_source_walk_exposes_missing_root() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let missing = directory.path().join("missing");
        let report = visible_source_files(&missing, &["rs"], ".rulewrightignore");

        verify_that!(report.files(), is_empty())?;
        verify_that!(report.failures(), not(is_empty()))?;

        Ok(())
    }
}
