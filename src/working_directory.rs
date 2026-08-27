//! Process working-directory discovery.

use std::io;

use crate::path::PathBuf;

/// Failure to determine the process working directory.
#[derive(Debug, thiserror::Error)]
#[error("failed to determine process working directory: {source}")]
pub(crate) struct Error {
    #[from]
    source: io::Error,
}

/// Return the process working directory.
///
/// # Errors
///
/// Returns an error when the operating system cannot provide the directory.
pub(crate) fn current() -> Result<PathBuf, Error> {
    let directory = std::env::current_dir()?;

    Ok(PathBuf::from(directory))
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::*;
    use crate::directory::{self, EntryKind};

    #[gtest]
    fn current_path_is_an_absolute_directory() -> Result<()> {
        let current = current().or_fail()?;
        let entry = directory::inspect(&current).or_fail()?.or_fail()?;

        verify_true!(current.is_absolute())?;

        verify_that!(entry.kind(), eq(EntryKind::Directory))
    }
}
