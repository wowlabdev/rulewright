//! Test-only scoped temporary directory ownership.

use crate::{
    error::{Error, Operation, Result},
    path::Path,
};

/// A unique temporary directory removed when dropped.
#[derive(Debug)]
pub(crate) struct Directory {
    inner: tempfile::TempDir,
}

impl Directory {
    /// Create a unique temporary directory.
    pub(crate) fn new() -> Result<Self> {
        let inner = tempfile::Builder::new()
            .prefix("rulewright-")
            .tempdir()
            .map_err(|source| {
                Error::new(
                    Operation::CreateDirectory,
                    Path::new(std::env::temp_dir().as_os_str()),
                    source,
                )
            })?;

        Ok(Self { inner })
    }

    /// Return the owned temporary root.
    #[must_use]
    pub(crate) fn path(&self) -> &Path {
        Path::new(self.inner.path().as_os_str())
    }
}
