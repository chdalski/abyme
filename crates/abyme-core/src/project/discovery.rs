//! Finding the project a directory lies in, and initializing one.

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use abyme_model::shown;
use serde::Deserialize;

use super::files::{
    MAX_PROJECT_FILE_BYTES, create_file, ensure_directory, ensure_local_dir, exists, io_error,
    read_limited,
};
use super::location::Location;
use super::{EmbeddingModel, EntryKind, Init, Project, ProjectError};

/// The version of the project file this crate reads and writes.
pub const PROJECT_FORMAT: u32 = 1;

/// How many characters of a parser message an error shows.
const DETAIL_LIMIT: usize = 200;

/// The project file as it lies on disk.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    format: u32,
    #[serde(default)]
    embedding_model: EmbeddingModel,
}

/// The effective user ID of this process: projects must belong to it.
pub fn current_user() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// Finds the project that `start` lies in, accepting it only when it
/// belongs to `user`.
pub fn discover(start: &Path, user: u32) -> Result<Project, ProjectError> {
    find(&canonical(start)?, user)?.ok_or(ProjectError::NotFound)
}

/// Makes `start` a project unless it lies in one; the project must belong
/// to `user`.
pub fn init(start: &Path, user: u32) -> Result<Init, ProjectError> {
    let start = canonical(start)?;
    if let Some(project) = find(&start, user)? {
        return Ok(Init::Existing(project));
    }
    if owner(&start)? != user {
        return Err(ProjectError::NotOwned { root: start });
    }
    // `.abyme/` first: Git ignores it, so a refused `abyme/` leaves nothing
    // a commit would pick up.
    ensure_local_dir(&start)?;
    ensure_directory(&start, Location::AbymeDir)?;
    create_file(
        &start,
        Location::ProjectFile,
        &format!("format = {PROJECT_FORMAT}\n"),
    )
    .map_err(io_error(Location::ProjectFile))?;
    Ok(Init::Created(Project {
        root: start,
        embedding_model: EmbeddingModel::default(),
    }))
}

/// Fails unless the root directory, `abyme/`, and `abyme/abyme.toml` all
/// belong to `user`; `owner_of` tells who owns an entry.
fn check_owners(
    root: &Path,
    user: u32,
    owner_of: impl Fn(&Path) -> Result<u32, ProjectError>,
) -> Result<(), ProjectError> {
    let entries = [
        root.to_path_buf(),
        root.join(Location::AbymeDir.relative()),
        root.join(Location::ProjectFile.relative()),
    ];
    for entry in &entries {
        if owner_of(entry)? != user {
            return Err(ProjectError::NotOwned {
                root: root.to_path_buf(),
            });
        }
    }
    Ok(())
}

/// Searches `start` and the directories above it for the nearest project.
/// A project found that belongs to another user fails the search rather
/// than letting it go on upwards.
fn find(start: &Path, user: u32) -> Result<Option<Project>, ProjectError> {
    for directory in start.ancestors() {
        match fs::symlink_metadata(directory.join(Location::ProjectFile.relative())) {
            Ok(_) => return accept(directory, user).map(Some),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) => {}
            Err(source) => {
                return Err(ProjectError::Search {
                    directory: directory.to_path_buf(),
                    source,
                });
            }
        }
    }
    Ok(None)
}

/// Accepts `root` as the project when it belongs to `user` and its project
/// file is valid.
fn accept(root: &Path, user: u32) -> Result<Project, ProjectError> {
    check_owners(root, user, owner)?;
    exists(root, Location::AbymeDir, EntryKind::Directory)?;
    exists(root, Location::ProjectFile, EntryKind::File)?;
    let text = read_limited(root, Location::ProjectFile, MAX_PROJECT_FILE_BYTES)?;
    let file: ProjectFile =
        toml::from_str(&text).map_err(|error| ProjectError::InvalidProjectFile {
            detail: shown(error.message(), DETAIL_LIMIT),
        })?;
    if file.format != PROJECT_FORMAT {
        return Err(ProjectError::InvalidProjectFile {
            detail: format!(
                "format {} is not supported, only {PROJECT_FORMAT}",
                file.format
            ),
        });
    }
    Ok(Project {
        root: root.to_path_buf(),
        embedding_model: file.embedding_model,
    })
}

/// The owner of the entry at `path`, without following a symbolic link.
fn owner(path: &Path) -> Result<u32, ProjectError> {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.uid())
        .map_err(|source| ProjectError::Search {
            directory: path.to_path_buf(),
            source,
        })
}

/// The absolute path of `start` without symbolic links, so that the search
/// upwards sees every directory above it.
fn canonical(start: &Path) -> Result<PathBuf, ProjectError> {
    fs::canonicalize(start).map_err(|source| ProjectError::Search {
        directory: start.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Who owns each entry: user 1, except `foreign`, which user 2 owns.
    fn owners_with(foreign: &str) -> impl Fn(&Path) -> Result<u32, ProjectError> {
        let foreign = PathBuf::from(foreign);
        move |path| Ok(if path == foreign { 2 } else { 1 })
    }

    #[rstest]
    #[case::root_directory_foreign("/r")]
    #[case::abyme_directory_foreign("/r/abyme")]
    #[case::project_file_foreign("/r/abyme/abyme.toml")]
    fn rejects_a_project_with_one_foreign_owner(#[case] foreign: &str) {
        let result = check_owners(Path::new("/r"), 1, owners_with(foreign));

        assert!(
            matches!(&result, Err(ProjectError::NotOwned { root }) if root == Path::new("/r")),
            "{result:?}"
        );
    }

    #[test]
    fn accepts_a_project_the_user_owns_entirely() {
        assert!(check_owners(Path::new("/r"), 1, owners_with("/elsewhere")).is_ok());
    }
}
