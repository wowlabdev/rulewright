//! Deterministic content checksums and snapshots of reviewed file sets.
//!
//! Tree capture intentionally accepts an explicit list instead of discovering a directory.
//! Callers retain control over ignore rules, discovery symlink policy, and the input boundary.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::{
    directory::{self, EntryKind},
    error::{Error as FilesystemError, Operation},
    path::{Component, Path, PathBuf},
};

const CHECKSUM_BYTES: usize = 32;
const HEX_PAIR_BYTES: usize = 2;
const ADJACENT_PAIR: usize = 2;
const SECOND_ITEM: usize = 1;
const CHECKSUM_HEX_LENGTH: usize = CHECKSUM_BYTES * HEX_PAIR_BYTES;
const NIBBLE_BITS: u32 = 4;
const HEX_ALPHA_OFFSET: u8 = 10;
const TREE_DOMAIN: &[u8] = b"rulewright:tree-snapshot:v1\0";

/// A BLAKE3 content checksum.
///
/// This type identifies cache inputs and generated artifacts.
/// It is not an authentication primitive.
/// Do not use it as a message authentication code or password hash.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Checksum([u8; CHECKSUM_BYTES]);

impl Checksum {
    /// Return the fixed-width binary representation.
    #[must_use]
    pub(crate) const fn as_bytes(&self) -> &[u8; CHECKSUM_BYTES] {
        &self.0
    }
}

impl fmt::Debug for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Checksum").field(&self.to_string()).finish()
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }

        Ok(())
    }
}

impl FromStr for Checksum {
    type Err = ParseChecksumError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        if value.len() != CHECKSUM_HEX_LENGTH {
            return Err(ParseChecksumError);
        }

        let mut bytes = [0_u8; CHECKSUM_BYTES];

        for (byte, pair) in bytes
            .iter_mut()
            .zip(value.as_bytes().chunks_exact(HEX_PAIR_BYTES))
        {
            let high = pair.first().copied().and_then(decode_hex);
            let low = pair.get(SECOND_ITEM).copied().and_then(decode_hex);
            let (Some(high), Some(low)) = (high, low) else {
                return Err(ParseChecksumError);
            };

            *byte = (high << NIBBLE_BITS) | low;
        }

        Ok(Self(bytes))
    }
}

impl Serialize for Checksum {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Checksum {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        // #rw(rust_map_err_pure_wrap) Serde requires conversion into the deserializer's error type.
        value.parse().map_err(D::Error::custom)
    }
}

/// Failure to parse the canonical hexadecimal representation of a checksum.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("a checksum must contain exactly 64 hexadecimal characters")]
pub(crate) struct ParseChecksumError;

/// Compute the BLAKE3 checksum of bytes already in memory.
#[must_use]
pub(crate) fn bytes(contents: impl AsRef<[u8]>) -> Checksum {
    Checksum(*blake3::hash(contents.as_ref()).as_bytes())
}

/// Compute the BLAKE3 checksum of a regular file.
///
/// The file is read as a stream rather than loaded entirely into memory.
///
/// # Errors
///
/// Returns an error when the path is missing, is not a regular file, or cannot be read.
/// Inspection does not follow the final symbolic link.
pub(crate) fn file(path: &Path) -> Result<Checksum> {
    checked_file(path).map(|file| file.checksum)
}

/// Compute the checksum of the currently running executable.
///
/// This gives caches a strong identity for the exact tool implementation.
///
/// # Errors
///
/// Returns an error when the executable path cannot be resolved or read.
pub(crate) fn current_executable() -> Result<Checksum> {
    let path = match std::env::current_exe() {
        Ok(path) => PathBuf::from(path),
        Err(source) => return Err(ErrorKind::CurrentExecutable(source).into()),
    };

    file(&path)
}

/// One regular file included in a [`TreeSnapshot`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct TreeEntry {
    relative_path: PathBuf,
    len: u64,
    checksum: Checksum,
    #[serde(skip)]
    components: Vec<Box<str>>,
}

impl TreeEntry {
    /// Return the normalized path relative to the snapshot root.
    #[must_use]
    pub(crate) fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Return the file content checksum.
    #[must_use]
    pub(crate) const fn checksum(&self) -> Checksum {
        self.checksum
    }
}

/// A deterministic snapshot of an explicitly reviewed set of regular files.
///
/// Entries are normalized and sorted by relative path.
/// The checksum includes a format version, relative paths, lengths, and content checksums.
/// Input order has no effect.
/// Adding, deleting, renaming, or editing a file changes the checksum.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct TreeSnapshot {
    checksum: Checksum,
    entries: Vec<TreeEntry>,
}

impl TreeSnapshot {
    /// Capture an explicit set of files beneath a lexical root.
    ///
    /// Paths normally come from one of [`crate::walk`]'s reviewed traversal policies.
    /// Relative components must be valid UTF-8 for portable checksums.
    ///
    /// Parent-directory symlinks are not resolved.
    /// This operation establishes cache identity, not a containment boundary.
    /// Use [`crate::containment`] first for paths originating from untrusted input.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, duplicate, missing, non-regular, or unreadable paths.
    pub(crate) fn capture<I, P>(root: &Path, paths: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut candidates = paths
            .into_iter()
            .map(|path| Candidate::new(root, path.as_ref()))
            .collect::<Result<Vec<_>>>()?;

        candidates.sort_by(|left, right| left.sort_key.cmp(&right.sort_key));

        let duplicate = candidates.windows(ADJACENT_PAIR).position(|pair| {
            pair.first()
                .zip(pair.get(SECOND_ITEM))
                .is_some_and(|(left, right)| left.sort_key == right.sort_key)
        });

        if let Some(index) = duplicate {
            let duplicate = candidates.remove(index + SECOND_ITEM);

            return Err(ErrorKind::DuplicatePath {
                path: duplicate.relative_path,
            }
            .into());
        }

        let mut entries = Vec::with_capacity(candidates.len());
        let mut total_bytes = 0_u64;

        for candidate in candidates {
            let captured = checked_file(&candidate.path)?;

            total_bytes = total_bytes
                .checked_add(captured.len)
                .ok_or(ErrorKind::InputTooLarge)?;
            entries.push(TreeEntry {
                relative_path: candidate.relative_path,
                len: captured.len,
                checksum: captured.checksum,
                components: candidate.sort_key,
            });
        }

        let checksum = tree_checksum(&entries)?;

        Ok(Self { checksum, entries })
    }

    /// Return the checksum of the complete tree snapshot.
    #[must_use]
    pub(crate) const fn checksum(&self) -> Checksum {
        self.checksum
    }

    /// Return entries in normalized relative-path order.
    #[must_use]
    pub(crate) fn entries(&self) -> &[TreeEntry] {
        &self.entries
    }
}

/// Failure to checksum a file or capture a tree snapshot.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub(crate) struct Error(#[from] ErrorKind);

#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    #[error(transparent)]
    Filesystem(#[from] FilesystemError),
    #[error("failed to resolve the current executable: {0}")]
    CurrentExecutable(#[source] std::io::Error),
    #[error("checksum path {path} is outside root {root}: {source}")]
    OutsideRoot {
        root: PathBuf,
        path: PathBuf,
        #[source]
        source: crate::path::StripPrefixError,
    },
    #[error("checksum path {path} is the snapshot root")]
    RootPath { path: PathBuf },
    #[error("checksum path {path} has a non-UTF-8 or non-normal relative component")]
    InvalidRelativePath { path: PathBuf },
    #[error("checksum path {path} appears more than once")]
    DuplicatePath { path: PathBuf },
    #[error("checksum file {path} does not exist")]
    MissingFile { path: PathBuf },
    #[error("checksum path {path} is not a regular file ({kind:?})")]
    NotRegularFile { path: PathBuf, kind: EntryKind },
    #[error("checksum input exceeds the supported length")]
    InputTooLarge,
}

impl From<FilesystemError> for Error {
    fn from(source: FilesystemError) -> Self {
        ErrorKind::Filesystem(source).into()
    }
}

/// Result returned by checksum operations.
pub(crate) type Result<T> = std::result::Result<T, Error>;

struct Candidate {
    path: PathBuf,
    relative_path: PathBuf,
    sort_key: Vec<Box<str>>,
}

impl Candidate {
    fn new(root: &Path, path: &Path) -> Result<Self> {
        let relative = path.strip_prefix(root).map_err(|source| {
            Error::from(ErrorKind::OutsideRoot {
                root: root.to_path_buf(),
                path: path.to_path_buf(),
                source,
            })
        })?;

        if relative.is_empty() {
            return Err(ErrorKind::RootPath {
                path: path.to_path_buf(),
            }
            .into());
        }

        let mut relative_path = PathBuf::new();
        let mut sort_key = Vec::new();

        for component in relative.components() {
            let Component::Normal(value) = component else {
                return Err(ErrorKind::InvalidRelativePath {
                    path: path.to_path_buf(),
                }
                .into());
            };
            let value = value
                .to_str()
                .ok_or_else(|| ErrorKind::InvalidRelativePath {
                    path: path.to_path_buf(),
                })?;

            relative_path.push(value);
            sort_key.push(value.into());
        }

        Ok(Self {
            path: path.to_path_buf(),
            relative_path,
            sort_key,
        })
    }
}

struct CapturedFile {
    checksum: Checksum,
    len: u64,
}

fn checked_file(path: &Path) -> Result<CapturedFile> {
    let Some(info) = directory::inspect_link(path)? else {
        return Err(ErrorKind::MissingFile {
            path: path.to_path_buf(),
        }
        .into());
    };

    if info.kind() != EntryKind::File {
        return Err(ErrorKind::NotRegularFile {
            path: path.to_path_buf(),
            kind: info.kind(),
        }
        .into());
    }

    let file = crate::file::open(path)?;
    let mut reader = CountingReader::new(file);
    let mut hasher = blake3::Hasher::new();

    hasher
        .update_reader(&mut reader)
        .map_err(|source| FilesystemError::new(Operation::ReadFile, path, source))?;

    Ok(CapturedFile {
        checksum: Checksum(*hasher.finalize().as_bytes()),
        len: reader.len(),
    })
}

struct CountingReader {
    file: std::fs::File,
    len: u64,
}

impl CountingReader {
    const fn new(file: std::fs::File) -> Self {
        Self { file, len: 0 }
    }

    const fn len(&self) -> u64 {
        self.len
    }
}

impl std::io::Read for CountingReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.file.read(buf)?;
        let read_len = match u64::try_from(read) {
            Ok(read_len) => read_len,
            Err(source) => return Err(std::io::Error::other(source)),
        };

        self.len = self
            .len
            .checked_add(read_len)
            .ok_or_else(|| std::io::Error::other("checksum input exceeds the supported length"))?;

        Ok(read)
    }
}

fn tree_checksum(entries: &[TreeEntry]) -> Result<Checksum> {
    let mut hasher = blake3::Hasher::new();

    hasher.update(TREE_DOMAIN);
    update_length(&mut hasher, entries.len())?;

    for entry in entries {
        update_length(&mut hasher, entry.components.len())?;

        for value in &entry.components {
            update_length(&mut hasher, value.len())?;
            hasher.update(value.as_bytes());
        }

        hasher.update(&entry.len.to_le_bytes());
        hasher.update(entry.checksum.as_bytes());
    }

    Ok(Checksum(*hasher.finalize().as_bytes()))
}

fn update_length(hasher: &mut blake3::Hasher, len: usize) -> Result<()> {
    let len = match u64::try_from(len) {
        Ok(len) => len,
        Err(_source) => return Err(ErrorKind::InputTooLarge.into()),
    };

    hasher.update(&len.to_le_bytes());

    Ok(())
}

const fn decode_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + HEX_ALPHA_OFFSET),
        b'A'..=b'F' => Some(value - b'A' + HEX_ALPHA_OFFSET),
        _ => None,
    }
}
