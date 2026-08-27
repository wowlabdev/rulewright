//! Directory mutation and symlink-aware inspection.

#[cfg(test)]
use crate::path::PathBuf;
use crate::{
    error::{Error, Operation, Result},
    path::Path,
};

/// Kind of entry observed without following its final symlink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub(crate) enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A symbolic link.
    Symlink,
    /// A platform-specific entry kind.
    Other,
}

/// Snapshot of one filesystem entry.
#[derive(Clone, Debug)]
pub(crate) struct EntryInfo {
    #[cfg(test)]
    path: PathBuf,
    kind: EntryKind,
}

impl EntryInfo {
    /// Return the entry kind.
    #[must_use]
    pub(crate) const fn kind(&self) -> EntryKind {
        self.kind
    }
}

/// Create a directory and every missing ancestor.
///
/// # Errors
///
/// Returns a contextual error when the directory cannot be ensured.
pub(crate) fn ensure(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .map_err(|source| Error::new(Operation::CreateDirectory, path, source))
}

/// Create exactly one directory.
///
/// # Errors
///
/// Returns a contextual error when the directory cannot be created.
#[cfg(test)]
pub(crate) fn create(path: &Path) -> Result<()> {
    std::fs::create_dir(path).map_err(|source| Error::new(Operation::CreateDirectory, path, source))
}

/// Inspect a path while following its final symlink.
///
/// A missing path is represented by `Ok(None)`.
///
/// # Errors
///
/// Returns a contextual error when metadata cannot be read.
pub(crate) fn inspect(path: &Path) -> Result<Option<EntryInfo>> {
    inspect_with(path, true)
}

/// Inspect a path without following its final symlink.
///
/// A missing path is represented by `Ok(None)`.
///
/// # Errors
///
/// Returns a contextual error when metadata cannot be read.
pub(crate) fn inspect_link(path: &Path) -> Result<Option<EntryInfo>> {
    inspect_with(path, false)
}

fn inspect_with(path: &Path, follow: bool) -> Result<Option<EntryInfo>> {
    let result = if follow {
        std::fs::metadata(path)
    } else {
        std::fs::symlink_metadata(path)
    };
    let metadata = match result {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(Error::new(Operation::Inspect, path, source)),
    };
    let file_type = metadata.file_type();
    let kind = if file_type.is_file() {
        EntryKind::File
    } else if file_type.is_dir() {
        EntryKind::Directory
    } else if file_type.is_symlink() {
        EntryKind::Symlink
    } else {
        EntryKind::Other
    };

    Ok(Some(EntryInfo {
        #[cfg(test)]
        path: path.to_path_buf(),
        kind,
    }))
}

/// Read and sort all immediate directory entries.
///
/// Entry kinds are captured without following final symlinks.
///
/// # Errors
///
/// Returns a contextual error for opening the directory or reading any entry.
#[cfg(test)]
pub(crate) fn entries(path: &Path) -> Result<Vec<EntryInfo>> {
    let entries = std::fs::read_dir(path)
        .map_err(|source| Error::new(Operation::ReadDirectory, path, source))?;
    let mut result = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|source| Error::new(Operation::ReadDirectory, path, source))?;
        let entry_path = PathBuf::from(entry.path());
        let info = inspect_link(&entry_path)?.ok_or_else(|| {
            Error::new(
                Operation::Inspect,
                &entry_path,
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "directory entry disappeared during inspection",
                ),
            )
        })?;

        result.push(info);
    }

    result.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(result)
}
