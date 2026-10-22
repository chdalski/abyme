//! The project on disk: finding it, initializing it, and reading and writing
//! its files.
//!
//! The files arrive through clones and pull requests, so they are untrusted:
//! they can be symbolic links, special files, or files of any size, and
//! another local user can place a project in a directory above the working
//! directory. So every path comes from [`Location`], a symbolic link at a
//! fixed path fails the call, entries that are not regular files are
//! ignored, every read is limited in size, and a project is used only when
//! it belongs to the user running Abyme.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use abyme_model::{BoardError, ElementError, shown};

use crate::apply::{ApplyError, Created, Snapshot};
use crate::operation::{LocalKey, Operation};

mod discovery;
mod files;
mod location;
mod store;

pub use discovery::PROJECT_FORMAT;
pub use files::{MAX_FILE_BYTES, MAX_PROJECT_FILE_BYTES};

/// How many characters of a path above the project an error shows.
const SHOWN_PATH_LIMIT: usize = 200;

/// An Abyme project: a directory that holds `abyme/abyme.toml` and belongs
/// to the user running Abyme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    root: PathBuf,
}

/// What [`Project::init`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Init {
    /// It created a project in the start directory.
    Created(Project),
    /// The start directory lies in a project already; nothing changed.
    Existing(Project),
}

/// Why a project cannot be found, initialized, read, or written. Paths
/// inside the project are relative to its root.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// Neither the start directory nor a directory above it holds
    /// `abyme/abyme.toml`.
    #[error(
        "no Abyme project in this directory or above it: run `abyme-mcp init` in the directory that is to become the project's root"
    )]
    NotFound,
    /// The project found belongs to another user, so it is not used.
    #[error(
        "the Abyme project in {} belongs to another user, so it is not used",
        shown_path(.root)
    )]
    NotOwned {
        /// The root of the project found.
        root: PathBuf,
    },
    /// A directory on the way to the project cannot be searched.
    #[error("{} cannot be searched for an Abyme project: {source}", shown_path(.directory))]
    Search {
        /// The directory that cannot be searched.
        directory: PathBuf,
        /// What went wrong.
        source: io::Error,
    },
    /// A fixed path holds a symbolic link or another kind of entry than
    /// expected.
    #[error("{} must be {expected}, not a symbolic link or anything else", .path.display())]
    WrongKind {
        /// The path, relative to the root.
        path: PathBuf,
        /// What it must be.
        expected: EntryKind,
    },
    /// A file is larger than its kind allows.
    #[error("{} is larger than {limit} bytes", .path.display())]
    TooLarge {
        /// The file, relative to the root.
        path: PathBuf,
        /// The largest size allowed.
        limit: usize,
    },
    /// A file is not valid UTF-8.
    #[error("{} is not valid UTF-8", .path.display())]
    NotUtf8 {
        /// The file, relative to the root.
        path: PathBuf,
    },
    /// The project file is not one this crate reads.
    #[error("abyme/abyme.toml: {detail}")]
    InvalidProjectFile {
        /// What is wrong with it, escaped and shortened.
        detail: String,
    },
    /// An element file cannot be read.
    #[error(transparent)]
    Element(#[from] ElementError),
    /// A board file cannot be read.
    #[error(transparent)]
    Board(#[from] BoardError),
    /// The request is rejected; nothing is written.
    #[error(transparent)]
    Rejected(#[from] ApplyError),
    /// The request would write a file larger than the reader accepts;
    /// nothing is written.
    #[error("{} would have {size} bytes, more than {limit}", .path.display())]
    WriteTooLarge {
        /// The file, relative to the root.
        path: PathBuf,
        /// The size it would have.
        size: usize,
        /// The largest size allowed.
        limit: usize,
    },
    /// Reading or writing a file failed.
    #[error("{}: {source}", .path.display())]
    Io {
        /// The file or directory, relative to the root.
        path: PathBuf,
        /// What went wrong.
        source: io::Error,
    },
}

/// What a fixed path must hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A directory.
    Directory,
    /// A regular file.
    File,
}

impl Project {
    /// Finds the project that `start` lies in: the nearest directory, from
    /// `start` upwards, that holds `abyme/abyme.toml`. Git repositories
    /// nested in a project do not stop the search.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::NotFound`] when no such directory exists,
    /// [`ProjectError::NotOwned`] when the nearest one, its `abyme/`, or its
    /// `abyme/abyme.toml` belongs to another user, and other errors when
    /// the project file is rejected or cannot be read.
    pub fn discover(start: &Path) -> Result<Self, ProjectError> {
        discovery::discover(start, discovery::current_user())
    }

    /// Makes `start` an Abyme project, unless it lies in one already:
    /// creates `abyme/abyme.toml` and `.abyme/`, which ignores itself.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Project::discover`] other than
    /// [`ProjectError::NotFound`], and [`ProjectError::NotOwned`] when
    /// `start` belongs to another user.
    pub fn init(start: &Path) -> Result<Init, ProjectError> {
        discovery::init(start, discovery::current_user())
    }

    /// The project's root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads every element and board of the project.
    ///
    /// # Errors
    ///
    /// Returns a [`ProjectError`] when a fixed path is a symbolic link, or
    /// an element or board file is rejected or cannot be read.
    pub fn snapshot(&self) -> Result<Snapshot, ProjectError> {
        store::read_snapshot(&self.root)
    }

    /// Applies `operations` to the project's files under the project's
    /// lock and returns the ID each key of the request received.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::Rejected`] or
    /// [`ProjectError::WriteTooLarge`] when the request is rejected, which
    /// writes nothing, and other errors when reading or writing fails.
    pub fn apply(
        &self,
        operations: &[Operation],
    ) -> Result<Vec<(LocalKey, Created)>, ProjectError> {
        store::apply_operations(&self.root, operations)
    }
}

impl fmt::Display for EntryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Directory => "a directory",
            Self::File => "a regular file",
        })
    }
}

/// Shows a path from outside the project, escaped and shortened.
fn shown_path(path: &Path) -> String {
    shown(&path.to_string_lossy(), SHOWN_PATH_LIMIT)
}

#[cfg(test)]
mod tests;
