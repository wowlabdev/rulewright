//! Lexical native-path types.

use std::{
    borrow::Borrow,
    convert::Infallible,
    ffi::{OsStr, OsString},
    fmt,
    ops::Deref,
    path::{
        Ancestors as NativeAncestors, Components as NativeComponents, Path as NativePath,
        PathBuf as NativePathBuf,
    },
    str::FromStr,
};

use ref_cast::RefCast;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A borrowed native path with lexical operations only.
///
/// Filesystem-query methods are deliberately absent.
/// Use this crate's operation modules to inspect or mutate the host filesystem.
#[derive(Eq, Hash, Ord, PartialEq, PartialOrd, RefCast)]
#[repr(transparent)]
pub struct Path(NativePath);

impl Path {
    /// Borrow a native path without allocating.
    #[must_use]
    pub fn new<S>(path: &S) -> &Self
    where
        S: AsRef<OsStr> + ?Sized,
    {
        Self::ref_cast(NativePath::new(path))
    }

    /// Return the underlying operating-system string.
    #[must_use]
    pub fn as_os_str(&self) -> &OsStr {
        self.0.as_os_str()
    }

    /// Return this path as UTF-8 when every byte is valid UTF-8.
    #[must_use]
    pub fn to_str(&self) -> Option<&str> {
        self.0.to_str()
    }

    /// Return a display adapter that replaces invalid UTF-8 lossily.
    #[must_use]
    pub fn display(&self) -> Display<'_> {
        Display(self)
    }

    /// Return this path as a potentially lossy string.
    #[must_use]
    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        self.0.to_string_lossy()
    }

    /// Return a portable slash-separated representation without losing path identity.
    #[must_use]
    pub(crate) fn to_slash(&self) -> Option<String> {
        self.0.to_str().map(normalize_relative)
    }

    /// Allocate an owned copy.
    #[must_use]
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf(self.0.to_path_buf())
    }

    /// Join a lexical path segment.
    #[must_use]
    pub fn join<S>(&self, path: S) -> PathBuf
    where
        S: AsRef<NativePath>,
    {
        PathBuf(self.0.join(path))
    }

    /// Return the parent path.
    #[must_use]
    pub fn parent(&self) -> Option<&Self> {
        self.0.parent().map(Self::ref_cast)
    }

    /// Iterate over this path and each lexical parent.
    #[must_use]
    pub fn ancestors(&self) -> Ancestors<'_> {
        Ancestors(self.0.ancestors())
    }

    /// Return the final component.
    #[must_use]
    pub fn file_name(&self) -> Option<&OsStr> {
        self.0.file_name()
    }

    /// Return the final component without its last extension.
    #[must_use]
    pub fn file_stem(&self) -> Option<&OsStr> {
        self.0.file_stem()
    }

    /// Return the final extension.
    #[must_use]
    pub fn extension(&self) -> Option<&OsStr> {
        self.0.extension()
    }

    /// Return a path with a different final component.
    #[must_use]
    pub fn with_file_name<S>(&self, file_name: S) -> PathBuf
    where
        S: AsRef<OsStr>,
    {
        PathBuf(self.0.with_file_name(file_name))
    }

    /// Return a path with a different extension.
    #[must_use]
    pub fn with_extension<S>(&self, extension: S) -> PathBuf
    where
        S: AsRef<OsStr>,
    {
        PathBuf(self.0.with_extension(extension))
    }

    /// Iterate over normalized lexical components.
    #[must_use]
    pub fn components(&self) -> Components<'_> {
        Components(self.0.components())
    }

    /// Report whether this path begins with `base` on component boundaries.
    #[must_use]
    pub fn starts_with<S>(&self, base: S) -> bool
    where
        S: AsRef<NativePath>,
    {
        self.0.starts_with(base)
    }

    /// Remove a component-aligned prefix.
    ///
    /// # Errors
    ///
    /// Returns an error when `base` is not a lexical prefix of this path.
    pub fn strip_prefix<S>(&self, base: S) -> Result<&Self, StripPrefixError>
    where
        S: AsRef<NativePath>,
    {
        let path = self.0.strip_prefix(base)?;

        Ok(Self::ref_cast(path))
    }

    /// Report whether the path is absolute for the current platform.
    #[must_use]
    pub fn is_absolute(&self) -> bool {
        self.0.is_absolute()
    }

    /// Report whether the path has no components.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }
}

/// Normalize a relative path string to Rulewright's platform-independent form.
#[must_use]
pub(crate) fn normalize_relative(value: &str) -> String {
    if cfg!(windows) {
        value.replace('\\', "/")
    } else {
        value.to_owned()
    }
}

impl AsRef<NativePath> for Path {
    fn as_ref(&self) -> &NativePath {
        &self.0
    }
}

impl AsRef<Path> for Path {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl AsRef<OsStr> for Path {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// An owned native path with lexical operations only.
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PathBuf(NativePathBuf);

impl PathBuf {
    /// Create an empty path.
    #[must_use]
    pub fn new() -> Self {
        Self(NativePathBuf::new())
    }

    /// Borrow this owned path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        self
    }

    /// Append a lexical path.
    pub fn push<S>(&mut self, path: S)
    where
        S: AsRef<NativePath>,
    {
        self.0.push(path);
    }

    /// Remove the final component.
    pub fn pop(&mut self) -> bool {
        self.0.pop()
    }

    /// Replace the final extension.
    pub fn set_extension<S>(&mut self, extension: S) -> bool
    where
        S: AsRef<OsStr>,
    {
        self.0.set_extension(extension)
    }

    /// Consume this path as an operating-system string.
    #[must_use]
    pub fn into_os_string(self) -> OsString {
        self.0.into_os_string()
    }
}

impl AsRef<NativePath> for PathBuf {
    fn as_ref(&self) -> &NativePath {
        &self.0
    }
}

impl AsRef<OsStr> for PathBuf {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl AsRef<Path> for PathBuf {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl Borrow<Path> for PathBuf {
    fn borrow(&self) -> &Path {
        self
    }
}

impl Deref for PathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        Path::ref_cast(self.0.as_path())
    }
}

impl fmt::Debug for PathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for PathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display().fmt(f)
    }
}

impl From<NativePathBuf> for PathBuf {
    fn from(path: NativePathBuf) -> Self {
        Self(path)
    }
}

impl From<&NativePath> for PathBuf {
    fn from(path: &NativePath) -> Self {
        Self(path.to_path_buf())
    }
}

impl From<&Path> for PathBuf {
    fn from(path: &Path) -> Self {
        path.to_path_buf()
    }
}

impl From<OsString> for PathBuf {
    fn from(path: OsString) -> Self {
        Self(NativePathBuf::from(path))
    }
}

impl From<&OsStr> for PathBuf {
    fn from(path: &OsStr) -> Self {
        Self(NativePathBuf::from(path))
    }
}

impl From<String> for PathBuf {
    fn from(path: String) -> Self {
        Self(NativePathBuf::from(path))
    }
}

impl From<&str> for PathBuf {
    fn from(path: &str) -> Self {
        Self(NativePathBuf::from(path))
    }
}

impl FromStr for PathBuf {
    type Err = Infallible;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        Ok(Self(NativePathBuf::from(path)))
    }
}

impl From<PathBuf> for OsString {
    fn from(path: PathBuf) -> Self {
        path.into_os_string()
    }
}

impl Serialize for PathBuf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        NativePathBuf::deserialize(deserializer).map(Self)
    }
}

/// One normalized lexical component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Component<'a> {
    /// A platform prefix such as a Windows drive or UNC share.
    Prefix(&'a OsStr),
    /// The root directory separator.
    Root,
    /// A current-directory component.
    Current,
    /// A parent-directory component.
    Parent,
    /// An ordinary file-name component.
    Normal(&'a OsStr),
}

impl Component<'_> {
    /// Return the operating-system string represented by this component.
    #[must_use]
    pub fn as_os_str(&self) -> &OsStr {
        match self {
            Self::Prefix(value) | Self::Normal(value) => value,
            Self::Root => std::path::MAIN_SEPARATOR_STR.as_ref(),
            Self::Current => ".".as_ref(),
            Self::Parent => "..".as_ref(),
        }
    }
}

/// Iterator over a path's normalized lexical components.
#[derive(Clone, Debug)]
pub struct Components<'a>(NativeComponents<'a>);

impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|component| match component {
            std::path::Component::Prefix(prefix) => Component::Prefix(prefix.as_os_str()),
            std::path::Component::RootDir => Component::Root,
            std::path::Component::CurDir => Component::Current,
            std::path::Component::ParentDir => Component::Parent,
            std::path::Component::Normal(value) => Component::Normal(value),
        })
    }
}

impl DoubleEndedIterator for Components<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|component| match component {
            std::path::Component::Prefix(prefix) => Component::Prefix(prefix.as_os_str()),
            std::path::Component::RootDir => Component::Root,
            std::path::Component::CurDir => Component::Current,
            std::path::Component::ParentDir => Component::Parent,
            std::path::Component::Normal(value) => Component::Normal(value),
        })
    }
}

/// Iterator over a path and its lexical parents.
#[derive(Clone, Debug)]
pub struct Ancestors<'a>(NativeAncestors<'a>);

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a Path;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Path::ref_cast)
    }
}

/// Failure to remove a lexical path prefix.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct StripPrefixError(#[from] std::path::StripPrefixError);

/// Display adapter for a native path.
#[derive(Clone, Copy, Debug)]
pub struct Display<'a>(&'a Path);

impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.0.display().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::*;

    #[gtest]
    fn lexical_api_preserves_components_and_parents() -> Result<()> {
        let path = Path::new("crates/fs/src/lib.rs");
        let components = path
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        verify_that!(
            components,
            elements_are![eq("crates"), eq("fs"), eq("src"), eq("lib.rs"),]
        )?;

        verify_that!(
            path.parent()
                .map(Path::display)
                .map(|value| value.to_string()),
            some(eq("crates/fs/src"))
        )
    }

    #[gtest]
    fn owned_path_dereferences_only_to_workspace_path() -> Result<()> {
        let path = PathBuf::from("crates").join("fs");

        verify_that!(path.as_path(), eq(Path::new("crates/fs")))?;
        verify_that!(path.file_name(), some(eq(OsStr::new("fs"))))?;

        verify_that!(path.strip_prefix("crates").or_fail()?, eq(Path::new("fs")))
    }

    #[gtest]
    fn owned_path_parses_lexically_without_filesystem_access() -> Result<()> {
        let expected = PathBuf::from("crates/fs/src");

        verify_that!("crates/fs/src".parse::<PathBuf>(), ok(eq(&expected)))
    }

    #[gtest]
    fn relative_paths_only_translate_native_windows_separators() -> Result<()> {
        let normalized = normalize_relative(r"crates\app\src\lib.rs");

        if cfg!(windows) {
            verify_eq!(normalized, "crates/app/src/lib.rs")
        } else {
            verify_eq!(normalized, r"crates\app\src\lib.rs")
        }
    }

    #[cfg(unix)]
    #[gtest]
    fn unix_backslashes_remain_part_of_the_path_component() -> Result<()> {
        verify_eq!(
            Path::new(r"module\name.rs").to_slash(),
            Some(r"module\name.rs".to_string())
        )
    }
}
