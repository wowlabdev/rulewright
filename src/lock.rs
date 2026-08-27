//! Process-scoped advisory file locks.

use crate::path::{Path, PathBuf};

/// An exclusive advisory lock released when dropped or when the process exits.
///
/// The lock file remains on disk.
/// Acquisition therefore cannot race with deletion and inode replacement.
#[derive(Debug)]
pub(crate) struct Lock {
    _file: std::fs::File,
}

impl Lock {
    /// Attempt to acquire an exclusive advisory lock without waiting.
    ///
    /// The lock is tied to the open file handle.
    /// A crashed process releases the operating-system lock automatically.
    /// Timestamp-based stale-lock recovery is neither required nor safe.
    ///
    /// # Errors
    ///
    /// Returns a busy error when another process owns the lock.
    /// Opening and locking failures are returned with path context.
    pub(crate) fn try_acquire(path: &Path) -> Result<Self, LockError> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|source| LockError::io(path, source))?;

        match fs4::FileExt::try_lock(&file) {
            Ok(()) => Ok(Self { _file: file }),
            Err(fs4::TryLockError::WouldBlock) => Err(LockError::busy(path)),
            Err(fs4::TryLockError::Error(source)) => Err(LockError::io(path, source)),
        }
    }
}

/// Failure to acquire an exclusive file lock.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub(crate) struct LockError(LockErrorKind);

#[derive(Debug, thiserror::Error)]
enum LockErrorKind {
    #[error("another process holds the filesystem lock {path}")]
    Busy { path: PathBuf },
    #[error("failed to acquire lock {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl LockError {
    fn busy(path: &Path) -> Self {
        Self(LockErrorKind::Busy {
            path: path.to_path_buf(),
        })
    }

    fn io(path: &Path, source: std::io::Error) -> Self {
        Self(LockErrorKind::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Report whether another process currently owns the lock.
    #[must_use]
    #[cfg(test)]
    pub(crate) const fn is_busy(&self) -> bool {
        matches!(self.0, LockErrorKind::Busy { .. })
    }
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::Lock;
    use crate::temporary::Directory;

    #[gtest]
    fn lock_is_exclusive_and_released_with_its_guard() -> Result<()> {
        let directory = Directory::new().or_fail()?;
        let path = directory.path().join("operation.lock");
        let first = Lock::try_acquire(&path).or_fail()?;

        verify_true!(Lock::try_acquire(&path).unwrap_err().is_busy())?;

        drop(first);

        verify_true!(Lock::try_acquire(&path).is_ok())
    }
}
