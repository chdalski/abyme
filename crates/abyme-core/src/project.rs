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

use abyme_model::{BoardError, CommitId, Element, ElementError, ElementId, Verifier, shown};

use crate::apply::{ApplyError, Created, Snapshot, Write};
use crate::operation::{LocalKey, Operation};
use crate::review::{Review, Unavailable, VerifyProblem, review, stamped};

mod discovery;
mod files;
mod git;
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
    embedding_model: EmbeddingModel,
}

/// The embedding model a project's search uses, from the key
/// `embedding_model` of its project file: a closed set of built-in models.
/// The names carry the model's version.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
pub enum EmbeddingModel {
    /// BGE small en v1.5, quantized, for English.
    #[default]
    #[serde(rename = "bge-small-en-v1.5")]
    BgeSmallEnV15,
    /// paraphrase-multilingual-MiniLM-L12-v2, quantized, for 50+ languages.
    #[serde(rename = "paraphrase-multilingual-minilm-l12-v2")]
    ParaphraseMultilingualMiniLmL12V2,
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
    /// An element cannot be verified; nothing is written.
    #[error(transparent)]
    NotVerified(#[from] VerifyProblem),
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

    /// The embedding model the project file chooses, or the default.
    #[must_use]
    pub const fn embedding_model(&self) -> EmbeddingModel {
        self.embedding_model
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

    /// Applies `operations` as [`Project::apply`] does, if `check` passes on
    /// the reading of the files the write starts from. So no writer that takes
    /// the project's lock can change the files between the check and the write.
    ///
    /// `check` runs under the lock, once: it must neither write through a
    /// `Project` nor take the lock, which would wait for itself. A refusal comes
    /// before any rejection of the operations.
    ///
    /// # Errors
    ///
    /// The outer error is one of [`Project::apply`]'s. The inner one is the
    /// refusal of `check`, which writes nothing under `abyme/`.
    pub fn apply_checked<R>(
        &self,
        operations: &[Operation],
        check: impl FnOnce(&Snapshot) -> Result<(), R>,
    ) -> Result<Result<Vec<(LocalKey, Created)>, R>, ProjectError> {
        store::apply_checked(&self.root, operations, check)
    }

    /// The review flags of the elements of `snapshot`, from what the
    /// project's Git repository reports. Git only reads; no anchor reaches
    /// its command line.
    ///
    /// # Errors
    ///
    /// Returns an [`Unavailable`] when the project lies in no Git
    /// repository, `git` cannot be run, or Git fails.
    pub fn review(&self, snapshot: &Snapshot) -> Result<Review, Unavailable> {
        let elements: Vec<git::AnchorAndStamp<'_>> = snapshot
            .elements
            .values()
            .map(|element| git::AnchorAndStamp {
                anchor: element.anchor(),
                stamp: element.verified().map(|stamp| &stamp.commit),
            })
            .collect();
        let facts = git::facts(&self.root, &elements)?;
        Ok(review(snapshot, &facts))
    }

    /// Stamps the element `id` as verified by `by` against the commit HEAD
    /// names in the repository its anchor lies in, nested or the project's,
    /// and returns that commit. Git is asked under the project's lock, right
    /// before the element file is written, and only the stamp changes.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::NotVerified`] when the element has no file,
    /// the repository has no commit, files under the element's anchor
    /// differ from HEAD or are untracked, its anchor lies in a nested
    /// repository that Abyme cannot read, or Git cannot be asked; nothing is
    /// written then. Other errors come from reading or writing the files.
    pub fn verify(&self, id: &ElementId, by: Verifier) -> Result<CommitId, ProjectError> {
        store::write_locked(&self.root, |snapshot| {
            let anchor = snapshot.elements.get(id).and_then(Element::anchor);
            let head = git::head(&self.root, anchor)
                .map_err(|reason| VerifyProblem::Unavailable {
                    element: id.clone(),
                    reason,
                })?
                .ok_or_else(|| VerifyProblem::UnreadRepository(id.clone()))?;
            let (element, commit) = stamped(snapshot, id, &head, by)?;
            Ok((vec![Write::Element(element)], commit))
        })
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
