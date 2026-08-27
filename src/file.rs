//! File reading, writing, and streaming.

use crate::{
    error::{Error, Operation, Result},
    path::Path,
};

/// Open a file for reading.
///
/// # Errors
///
/// Returns a contextual error when the file cannot be opened.
pub(crate) fn open(path: &Path) -> Result<std::fs::File> {
    std::fs::File::open(path).map_err(|source| Error::new(Operation::OpenFile, path, source))
}

/// Read an entire file as bytes.
///
/// # Errors
///
/// Returns a contextual error when the file cannot be read.
pub(crate) fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::new(Operation::ReadFile, path, source))
}

/// Read an entire UTF-8 text file.
///
/// # Errors
///
/// Returns a contextual error for I/O failures or invalid UTF-8.
pub(crate) fn read_text(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| Error::new(Operation::ReadFile, path, source))
}

/// Read an entire UTF-8 text file when it exists.
///
/// # Errors
///
/// Returns a contextual error for failures other than a missing path.
pub(crate) fn read_text_if_exists(path: &Path) -> Result<Option<String>> {
    match read_text(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.is_not_found() => Ok(None),
        Err(error) => Err(error),
    }
}

/// Replace a file's contents directly.
///
/// Use [`crate::atomic::replace`] when interruption safety matters.
///
/// # Errors
///
/// Returns a contextual error when the contents cannot be written.
#[cfg(test)]
pub(crate) fn write_bytes(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(path, contents).map_err(|source| Error::new(Operation::WriteFile, path, source))
}

/// Replace a UTF-8 text file's contents directly.
///
/// # Errors
///
/// Returns a contextual error when the contents cannot be written.
#[cfg(test)]
pub(crate) fn write_text(path: &Path, contents: &str) -> Result<()> {
    write_bytes(path, contents)
}
