// #rw(file: rust_ambient_syscall) Durable filesystem commits must synchronize the containing directory through the native filesystem boundary.

//! Interruption-safe file replacement.

use std::io::Write as _;

use crate::{
    error::{Error, Operation, Result},
    path::Path,
};

/// Atomically replace a file with fully flushed bytes.
///
/// The temporary file is created beside the destination.
/// This prevents the commit from crossing filesystems.
/// The destination's parent must already exist.
///
/// # Errors
///
/// Returns a contextual error when temporary creation fails.
/// It also reports writing, synchronization, and commit failures.
pub(crate) fn replace(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    let parent = destination_parent(path)?;
    let permissions = match std::fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => return Err(Error::new(Operation::Inspect, path, source)),
    };
    let temporary = prepare(parent, path, contents, permissions)?;

    commit(temporary, parent, path, CommitMode::Replace)
}

/// Atomically create a file without replacing an existing entry.
///
/// Complete contents are flushed to a temporary file beside the destination.
/// The file is then committed without replacing an existing entry.
/// The destination's parent must already exist.
///
/// # Errors
///
/// Returns an already-exists error when the destination is present.
/// Other creation, writing, synchronization, and commit failures are contextual.
pub(crate) fn create(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    let parent = destination_parent(path)?;
    let temporary = prepare(parent, path, contents, None)?;

    commit(temporary, parent, path, CommitMode::Create)
}

#[derive(Clone, Copy, Debug)]
enum CommitMode {
    Create,
    Replace,
}

fn destination_parent(path: &Path) -> Result<&Path> {
    path.parent().ok_or_else(|| {
        Error::new(
            Operation::CreateFile,
            path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "atomic destination has no parent",
            ),
        )
    })
}

fn prepare(
    parent: &Path,
    path: &Path,
    contents: impl AsRef<[u8]>,
    permissions: Option<std::fs::Permissions>,
) -> Result<tempfile::NamedTempFile> {
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|source| Error::new(Operation::CreateFile, path, source))?;

    temporary
        .write_all(contents.as_ref())
        .map_err(|source| Error::new(Operation::WriteFile, path, source))?;
    temporary
        .as_file_mut()
        .flush()
        .map_err(|source| Error::new(Operation::WriteFile, path, source))?;

    if let Some(permissions) = permissions {
        temporary
            .as_file()
            .set_permissions(permissions)
            .map_err(|source| Error::new(Operation::WriteFile, path, source))?;
    }

    temporary
        .as_file()
        .sync_all()
        .map_err(|source| Error::new(Operation::Sync, path, source))?;

    Ok(temporary)
}

fn commit(
    temporary: tempfile::NamedTempFile,
    parent: &Path,
    path: &Path,
    mode: CommitMode,
) -> Result<()> {
    let committed = match mode {
        CommitMode::Create => temporary.persist_noclobber(path),
        CommitMode::Replace => temporary.persist(path),
    }
    .map_err(|error| Error::new(Operation::Rename, path, error.error))?;

    committed
        .sync_all()
        .map_err(|source| Error::new(Operation::Sync, path, source))?;

    sync_parent(parent, path)
}

#[cfg(unix)]
pub(crate) fn sync_parent(parent: &Path, path: &Path) -> Result<()> {
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| Error::new(Operation::Sync, path, source))
}

#[cfg(not(unix))]
pub(crate) fn sync_parent(_parent: &Path, _path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::{create, replace};
    use crate::{directory, file, temporary::Directory};

    #[gtest]
    fn replace_commits_exact_bytes_without_visible_temporary_entries() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let path = directory.path().join("artifact.txt");

        replace(&path, "first").or_fail()?;
        replace(&path, "second").or_fail()?;

        verify_that!(file::read_text(&path).or_fail()?, eq("second"))?;

        verify_that!(directory::entries(directory.path()).or_fail()?, len(eq(1)))
    }

    #[gtest]
    fn create_commits_once_without_clobbering() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let path = directory.path().join("config.toml");

        create(&path, "first").or_fail()?;
        let error = create(&path, "second").unwrap_err();

        verify_true!(error.is_already_exists())?;
        verify_that!(file::read_text(&path).or_fail()?, eq("first"))?;

        verify_that!(directory::entries(directory.path()).or_fail()?, len(eq(1)))
    }

    #[cfg(unix)]
    #[gtest]
    fn replace_preserves_unix_permissions() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = Directory::new().or_fail()?;
        let path = directory.path().join("script.sh");

        file::write_text(&path, "old").or_fail()?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o751)).or_fail()?;
        replace(&path, "new").or_fail()?;

        verify_eq!(
            std::fs::metadata(path).or_fail()?.permissions().mode() & 0o777,
            0o751
        )
    }
}
