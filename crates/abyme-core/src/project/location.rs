//! Every path Abyme touches in a project.

use std::path::PathBuf;

use abyme_model::{BoardId, ElementId};

/// The ending of an element file's name.
pub const ELEMENT_SUFFIX: &str = ".md";

/// The ending of a board file's name.
pub const BOARD_SUFFIX: &str = ".board.json";

/// A path in a project, relative to its root. No variant carries free text,
/// so a path can only come from fixed names, validated IDs, and numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location<'id> {
    /// `abyme/`, the Abyme format in Git.
    AbymeDir,
    /// `abyme/abyme.toml`, which makes a directory an Abyme project.
    ProjectFile,
    /// `abyme/elements/`.
    ElementsDir,
    /// `abyme/boards/`.
    BoardsDir,
    /// An element file in `abyme/elements/`.
    Element(&'id ElementId),
    /// A board file in `abyme/boards/`.
    Board(&'id BoardId),
    /// `.abyme/`, which Git ignores.
    LocalDir,
    /// `.abyme/.gitignore`, which makes Git ignore `.abyme/`.
    IgnoreFile,
    /// `.abyme/lock`, the advisory lock of every writer.
    Lock,
    /// `.abyme/tmp/`, where files are written before their rename.
    TempDir,
    /// A temporary file in `.abyme/tmp/`, named by the writing process and
    /// a counter.
    Temp {
        /// The ID of the writing process.
        process: u32,
        /// The process's count of temporary files.
        counter: u64,
    },
}

impl Location<'_> {
    /// The path relative to the project's root.
    pub fn relative(self) -> PathBuf {
        match self {
            Self::AbymeDir => PathBuf::from("abyme"),
            Self::ProjectFile => Self::AbymeDir.relative().join("abyme.toml"),
            Self::ElementsDir => Self::AbymeDir.relative().join("elements"),
            Self::BoardsDir => Self::AbymeDir.relative().join("boards"),
            Self::Element(id) => Self::ElementsDir
                .relative()
                .join(format!("{id}{ELEMENT_SUFFIX}")),
            Self::Board(id) => Self::BoardsDir
                .relative()
                .join(format!("{id}{BOARD_SUFFIX}")),
            Self::LocalDir => PathBuf::from(".abyme"),
            Self::IgnoreFile => Self::LocalDir.relative().join(".gitignore"),
            Self::Lock => Self::LocalDir.relative().join("lock"),
            Self::TempDir => Self::LocalDir.relative().join("tmp"),
            Self::Temp { process, counter } => Self::TempDir
                .relative()
                .join(format!("{process}-{counter}")),
        }
    }
}
