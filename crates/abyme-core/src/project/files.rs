//! Checking, reading, and creating the entries at fixed paths, without
//! following symbolic links and without reading more than a limit.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write as _};
use std::path::Path;

use super::location::Location;
use super::{EntryKind, ProjectError};

/// The largest element or board file, in bytes.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;

/// The largest project file, `abyme/abyme.toml`, in bytes.
pub const MAX_PROJECT_FILE_BYTES: usize = 64 * 1024;

/// Whether an entry of the expected kind exists at `location`: `false` when
/// nothing is there, an error when a symbolic link or another kind of entry
/// is.
pub fn exists(root: &Path, location: Location, expected: EntryKind) -> Result<bool, ProjectError> {
    match fs::symlink_metadata(root.join(location.relative())) {
        Ok(metadata) => {
            let kind = metadata.file_type();
            let matches = match expected {
                EntryKind::Directory => kind.is_dir(),
                EntryKind::File => kind.is_file(),
            };
            if matches {
                Ok(true)
            } else {
                Err(ProjectError::WrongKind {
                    path: location.relative(),
                    expected,
                })
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(io_error(location)(source)),
    }
}

/// Creates the directory at `location` unless it exists; its parent must
/// exist already.
pub fn ensure_directory(root: &Path, location: Location) -> Result<(), ProjectError> {
    if exists(root, location, EntryKind::Directory)? {
        return Ok(());
    }
    match fs::create_dir(root.join(location.relative())) {
        Ok(()) => Ok(()),
        // Another writer created it in the meantime; it must still be a
        // directory.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            exists(root, location, EntryKind::Directory).map(|_| ())
        }
        Err(source) => Err(io_error(location)(source)),
    }
}

/// Creates `.abyme/` with the `.gitignore` that makes Git ignore it,
/// unless they exist; an existing `.gitignore` stays as it is.
pub fn ensure_local_dir(root: &Path) -> Result<(), ProjectError> {
    ensure_directory(root, Location::LocalDir)?;
    match create_file(root, Location::IgnoreFile, "*\n") {
        // Whatever lies there, from the repository or another writer that
        // got there first, stays.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        result => result.map_err(io_error(Location::IgnoreFile)),
    }
}

/// Creates the file at `location` with `content`, failing with
/// [`io::ErrorKind::AlreadyExists`] if any entry is there already.
pub fn create_file(root: &Path, location: Location, content: &str) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(location.relative()))
        .and_then(|mut file| file.write_all(content.as_bytes()))
}

/// Reads the file at `location` as UTF-8, failing when it has more than
/// `limit` bytes. The caller has checked that it is a regular file.
pub fn read_limited(root: &Path, location: Location, limit: usize) -> Result<String, ProjectError> {
    let mut bytes = Vec::new();
    let allowed = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    File::open(root.join(location.relative()))
        .and_then(|file| file.take(allowed).read_to_end(&mut bytes))
        .map_err(io_error(location))?;
    if bytes.len() > limit {
        return Err(ProjectError::TooLarge {
            path: location.relative(),
            limit,
        });
    }
    String::from_utf8(bytes).map_err(|_| ProjectError::NotUtf8 {
        path: location.relative(),
    })
}

/// Turns an I/O error at `location` into a [`ProjectError`].
pub fn io_error(location: Location) -> impl Fn(io::Error) -> ProjectError {
    move |source| ProjectError::Io {
        path: location.relative(),
        source,
    }
}
