use std::collections::BTreeMap;

use crate::{
    atomic,
    checksum::{self, Checksum},
    lock::{Lock, LockError},
    path::{Path, PathBuf},
};

pub(crate) type SourceSnapshots = BTreeMap<String, Checksum>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ApplyError {
    #[error(transparent)]
    Filesystem(#[from] crate::error::Error),
    #[error(transparent)]
    Lock(#[from] LockError),
    #[error("fix plan for {0} has no source snapshot")]
    MissingSnapshot(PathBuf),
    #[error("{0} changed after it was analyzed; no fixes were applied")]
    StaleSource(PathBuf),
    #[error(
        "invalid fix range {start_line}..={end_line} for {path} ({line_count} source line(s)); no fixes were applied"
    )]
    InvalidRange {
        path: PathBuf,
        start_line: usize,
        end_line: usize,
        line_count: usize,
    },
    #[error("{0} contains mixed or unsupported line endings; no fixes were applied")]
    MixedLineEndings(PathBuf),
    #[error("fixes would make {path} invalid {language}: {message}; no fixes were applied")]
    InvalidSyntax {
        path: PathBuf,
        language: &'static str,
        message: String,
    },
}

/// An auto-fix: replace a line range with new text.
#[derive(Clone, Debug)]
pub struct Fix {
    pub start_line: usize,
    pub end_line: usize,
    pub replacement: String,
}

/// One whole-file replacement produced by a rust-analyzer syntax-tree edit.
#[derive(Clone, Debug)]
pub(crate) struct TreeFix {
    pub(crate) rel: String,
    pub(crate) rule: &'static str,
    pub(crate) replacement: String,
}

impl Fix {
    pub fn replace_line(line: usize, new: impl Into<String>) -> Self {
        Fix {
            start_line: line,
            end_line: line,
            replacement: new.into(),
        }
    }

    pub fn replace_lines(
        start_line: usize,
        end_line: usize,
        replacement: impl Into<String>,
    ) -> Self {
        Fix {
            start_line,
            end_line,
            replacement: replacement.into(),
        }
    }

    #[must_use]
    pub fn delete(start_line: usize, end_line: usize) -> Self {
        Fix {
            start_line,
            end_line,
            replacement: String::new(),
        }
    }
}

/// Apply `(rel_path, fix)` auto-fixes under a `.rulewright.lock`.
pub(crate) fn apply_fixes(
    fixes: &[(String, Fix)],
    snapshots: &SourceSnapshots,
    root: &Path,
) -> Result<usize, ApplyError> {
    let mut by_file: BTreeMap<&str, Vec<&Fix>> = BTreeMap::new();

    for (rel, fix) in fixes {
        by_file.entry(rel).or_default().push(fix);
    }

    let lock_path = root.join(".rulewright.lock");
    let _lock = Lock::try_acquire(&lock_path)?;
    let sources = verified_sources(by_file.keys().copied(), snapshots, root)?;

    validate_line_fix_plan(&by_file, &sources, root)?;

    let mut total = 0;
    let mut replacements = Vec::new();

    for (rel, mut file_fixes) in by_file {
        let path = root.join(rel);
        let Some(contents) = sources.get(rel) else {
            return Err(ApplyError::MissingSnapshot(path));
        };
        let mut lines: Vec<String> = contents.lines().map(String::from).collect();
        let mut file_total = 0;

        file_fixes.sort_by_key(|a| std::cmp::Reverse(a.start_line));

        // Bottom-to-top single pass: fixes at higher lines can't shift lower ones.
        let mut lowest_touched = usize::MAX;

        for fix in file_fixes {
            let start = fix.start_line - 1;
            let end = fix.end_line;

            if end > lowest_touched {
                continue;
            }

            if fix.replacement.is_empty() {
                lines.drain(start..end);
            } else {
                let new_lines: Vec<String> = fix.replacement.lines().map(String::from).collect();

                lines.splice(start..end, new_lines);
            }

            lowest_touched = start;
            file_total += 1;
        }

        if file_total == 0 {
            continue;
        }

        let newline = line_ending(contents, &path)?;
        let mut output = lines.join(newline);

        if contents.ends_with('\n') {
            output.push_str(newline);
        }

        validate_syntax(&path, contents, &output)?;
        replacements.push((path, output));
        total += file_total;
    }

    for (path, replacement) in replacements {
        atomic::replace(&path, replacement.as_bytes())?;
    }

    Ok(total)
}

/// Apply independent whole-file tree edits for one rule under the rulewright lock.
pub(crate) fn apply_tree_fixes(
    fixes: &[TreeFix],
    snapshots: &SourceSnapshots,
    root: &Path,
) -> Result<usize, ApplyError> {
    let lock_path = root.join(".rulewright.lock");
    let _lock = Lock::try_acquire(&lock_path)?;
    let sources = verified_sources(fixes.iter().map(|fix| fix.rel.as_str()), snapshots, root)?;

    for fix in fixes {
        let path = root.join(&fix.rel);
        let Some(contents) = sources.get(&fix.rel) else {
            return Err(ApplyError::MissingSnapshot(path));
        };

        line_ending(contents, &path)?;
    }

    let mut replacements = Vec::with_capacity(fixes.len());

    for fix in fixes {
        let path = root.join(&fix.rel);
        let Some(contents) = sources.get(&fix.rel) else {
            return Err(ApplyError::MissingSnapshot(path));
        };
        let replacement = with_line_ending(&fix.replacement, line_ending(contents, &path)?);

        validate_syntax(&path, contents, &replacement)?;
        replacements.push((path, replacement));
    }

    for (path, replacement) in replacements {
        atomic::replace(&path, replacement.as_bytes())?;
    }

    Ok(fixes.len())
}

fn validate_syntax(path: &Path, original: &str, replacement: &str) -> Result<(), ApplyError> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => {
            let original =
                ra_ap_syntax::SourceFile::parse(original, ra_ap_syntax::Edition::Edition2024);

            if !original.errors().is_empty() {
                return Ok(());
            }

            let parsed =
                ra_ap_syntax::SourceFile::parse(replacement, ra_ap_syntax::Edition::Edition2024);

            if let Some(error) = parsed.errors().first() {
                return Err(ApplyError::InvalidSyntax {
                    path: path.to_path_buf(),
                    language: "Rust",
                    message: error.to_string(),
                });
            }
        }
        Some("toml") => {
            if toml::from_str::<toml::Value>(original).is_ok()
                && let Err(error) = toml::from_str::<toml::Value>(replacement)
            {
                return Err(ApplyError::InvalidSyntax {
                    path: path.to_path_buf(),
                    language: "TOML",
                    message: error.to_string(),
                });
            }
        }
        _ => {}
    }

    Ok(())
}

fn verified_sources<'a>(
    rels: impl Iterator<Item = &'a str>,
    snapshots: &SourceSnapshots,
    root: &Path,
) -> Result<BTreeMap<String, String>, ApplyError> {
    let mut sources = BTreeMap::new();

    for rel in rels {
        let path = root.join(rel);
        let Some(expected) = snapshots.get(rel) else {
            return Err(ApplyError::MissingSnapshot(path));
        };
        let contents = crate::file::read_text(&path)?;

        if checksum::bytes(&contents) != *expected {
            return Err(ApplyError::StaleSource(path));
        }

        sources.insert(rel.to_owned(), contents);
    }

    Ok(sources)
}

fn validate_line_fix_plan(
    by_file: &BTreeMap<&str, Vec<&Fix>>,
    sources: &BTreeMap<String, String>,
    root: &Path,
) -> Result<(), ApplyError> {
    for (rel, fixes) in by_file {
        let path = root.join(rel);
        let Some(contents) = sources.get(*rel) else {
            return Err(ApplyError::MissingSnapshot(path));
        };
        let line_count = contents.lines().count();

        line_ending(contents, &path)?;

        for fix in fixes {
            if fix.start_line == 0 || fix.end_line < fix.start_line || fix.end_line > line_count {
                return Err(ApplyError::InvalidRange {
                    path,
                    start_line: fix.start_line,
                    end_line: fix.end_line,
                    line_count,
                });
            }
        }
    }

    Ok(())
}

fn line_ending(contents: &str, path: &Path) -> Result<&'static str, ApplyError> {
    let bytes = contents.as_bytes();
    let mut saw_lf = false;
    let mut saw_crlf = false;
    let mut index = 0;

    while index < bytes.len() {
        let Some(&byte) = bytes.get(index) else {
            break;
        };

        match byte {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                saw_crlf = true;
                index += 2;
            }
            b'\r' => return Err(ApplyError::MixedLineEndings(path.to_path_buf())),
            b'\n' => {
                saw_lf = true;
                index += 1;
            }
            _ => index += 1,
        }
    }

    if saw_lf && saw_crlf {
        Err(ApplyError::MixedLineEndings(path.to_path_buf()))
    } else if saw_crlf {
        Ok("\r\n")
    } else {
        Ok("\n")
    }
}

fn with_line_ending(contents: &str, newline: &str) -> String {
    let normalized = contents.replace("\r\n", "\n");

    if newline == "\n" {
        normalized
    } else {
        normalized.replace('\n', newline)
    }
}

#[cfg(test)]
#[path = "fix_validation_tests.rs"]
mod validation_tests;

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::*;
    use crate::{file, temporary::Directory};

    #[gtest]
    fn line_fixes_preserve_crlf() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let path = directory.path().join("source.rs");
        let original = "first\r\nsecond\r\n";

        file::write_text(&path, original).or_fail()?;
        let snapshots =
            SourceSnapshots::from([("source.rs".to_string(), checksum::bytes(original))]);
        let applied = apply_fixes(
            &[("source.rs".to_string(), Fix::replace_line(2, "changed"))],
            &snapshots,
            directory.path(),
        )
        .or_fail()?;

        verify_eq!(applied, 1)?;

        verify_eq!(file::read_text(&path).or_fail()?, "first\r\nchanged\r\n")
    }

    #[gtest]
    fn stale_source_aborts_before_any_file_is_changed() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let first = directory.path().join("first.rs");
        let second = directory.path().join("second.rs");

        file::write_text(&first, "current\n").or_fail()?;
        file::write_text(&second, "current\n").or_fail()?;
        let snapshots = SourceSnapshots::from([
            ("first.rs".to_string(), checksum::bytes("stale\n")),
            ("second.rs".to_string(), checksum::bytes("current\n")),
        ]);
        let error = apply_fixes(
            &[
                ("first.rs".to_string(), Fix::replace_line(1, "changed")),
                ("second.rs".to_string(), Fix::replace_line(1, "changed")),
            ],
            &snapshots,
            directory.path(),
        )
        .unwrap_err();

        verify_that!(
            error.to_string(),
            contains_substring("changed after it was analyzed")
        )?;
        verify_eq!(file::read_text(&first).or_fail()?, "current\n")?;

        verify_eq!(file::read_text(&second).or_fail()?, "current\n")
    }

    #[gtest]
    fn lock_failure_is_reported_as_an_error() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let path = directory.path().join("source.rs");

        file::write_text(&path, "old\n").or_fail()?;
        let snapshots =
            SourceSnapshots::from([("source.rs".to_string(), checksum::bytes("old\n"))]);
        let _lock = Lock::try_acquire(&directory.path().join(".rulewright.lock")).or_fail()?;
        let error = apply_fixes(
            &[("source.rs".to_string(), Fix::replace_line(1, "new"))],
            &snapshots,
            directory.path(),
        )
        .unwrap_err();

        verify_that!(error.to_string(), contains_substring("another process"))?;

        verify_eq!(file::read_text(&path).or_fail()?, "old\n")
    }

    #[gtest]
    fn malformed_range_aborts_before_any_file_is_changed() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let first = directory.path().join("first.rs");
        let second = directory.path().join("second.rs");

        file::write_text(&first, "first\n").or_fail()?;
        file::write_text(&second, "second\n").or_fail()?;
        let snapshots = SourceSnapshots::from([
            ("first.rs".to_string(), checksum::bytes("first\n")),
            ("second.rs".to_string(), checksum::bytes("second\n")),
        ]);
        let error = apply_fixes(
            &[
                ("first.rs".to_string(), Fix::replace_line(1, "changed")),
                ("second.rs".to_string(), Fix::replace_lines(0, 1, "invalid")),
            ],
            &snapshots,
            directory.path(),
        )
        .unwrap_err();

        verify_that!(error.to_string(), contains_substring("invalid fix range"))?;
        verify_eq!(file::read_text(&first).or_fail()?, "first\n")?;

        verify_eq!(file::read_text(&second).or_fail()?, "second\n")
    }

    #[gtest]
    fn line_fix_rejects_mixed_endings_without_mutation() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let path = directory.path().join("source.rs");
        let original = "first\r\nsecond\n";

        file::write_text(&path, original).or_fail()?;
        let snapshots =
            SourceSnapshots::from([("source.rs".to_string(), checksum::bytes(original))]);
        let error = apply_fixes(
            &[("source.rs".to_string(), Fix::replace_line(1, "changed"))],
            &snapshots,
            directory.path(),
        )
        .unwrap_err();

        verify_that!(error.to_string(), contains_substring("mixed"))?;

        verify_eq!(file::read_text(&path).or_fail()?, original)
    }

    #[gtest]
    fn tree_fix_rejects_mixed_endings_without_mutation() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let path = directory.path().join("source.rs");
        let original = "first\r\nsecond\n";

        file::write_text(&path, original).or_fail()?;
        let snapshots =
            SourceSnapshots::from([("source.rs".to_string(), checksum::bytes(original))]);
        let error = apply_tree_fixes(
            &[TreeFix {
                rel: "source.rs".to_string(),
                rule: "fixture",
                replacement: "changed\n".to_string(),
            }],
            &snapshots,
            directory.path(),
        )
        .unwrap_err();

        verify_that!(error.to_string(), contains_substring("mixed"))?;

        verify_eq!(file::read_text(&path).or_fail()?, original)
    }
}
