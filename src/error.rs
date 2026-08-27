//! Contextual filesystem errors.

use std::{fmt, io};

use crate::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Operation {
    CreateDirectory,
    CreateFile,
    Inspect,
    OpenFile,
    #[cfg(test)]
    ReadDirectory,
    ReadFile,
    Rename,
    Sync,
    WriteFile,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::CreateDirectory => "create directory",
            Self::CreateFile => "create file",
            Self::Inspect => "inspect",
            Self::OpenFile => "open file",
            #[cfg(test)]
            Self::ReadDirectory => "read directory",
            Self::ReadFile => "read file",
            Self::Rename => "rename",
            Self::Sync => "sync",
            Self::WriteFile => "write file",
        };

        f.write_str(value)
    }
}

/// A filesystem failure with the affected path attached.
#[derive(Debug, thiserror::Error)]
#[error("failed to {operation} {path}: {source}")]
pub(crate) struct Error {
    operation: Operation,
    path: PathBuf,
    #[source]
    source: io::Error,
}

impl Error {
    pub(crate) fn new(operation: Operation, path: &Path, source: io::Error) -> Self {
        Self {
            operation,
            path: path.to_path_buf(),
            source,
        }
    }

    /// Return the primary path involved in the failed operation.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Report whether the target did not exist.
    #[must_use]
    pub(crate) fn is_not_found(&self) -> bool {
        self.source.kind() == io::ErrorKind::NotFound
    }

    /// Report whether the target already existed.
    #[must_use]
    pub(crate) fn is_already_exists(&self) -> bool {
        self.source.kind() == io::ErrorKind::AlreadyExists
    }
}

/// Result returned by filesystem operations.
pub(crate) type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use std::{error::Error as _, io};

    use googletest::prelude::*;

    use super::{Error, Operation};
    use crate::path::Path;

    #[gtest]
    fn single_path_error_preserves_context_source_and_predicate() -> Result<()> {
        let error = Error::new(
            Operation::ReadFile,
            Path::new("config.toml"),
            io::Error::new(io::ErrorKind::NotFound, "missing"),
        );

        verify_eq!(
            error.to_string(),
            "failed to read file config.toml: missing"
        )?;
        verify_eq!(error.path(), Path::new("config.toml"))?;
        verify_true!(error.is_not_found())?;

        verify_true!(error.source().is_some())
    }
}
