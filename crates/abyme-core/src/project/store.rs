//! Reading the elements and boards, and writing a request's changes under
//! the project's lock.

use std::collections::BTreeMap;
use std::fs::{self, DirEntry, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use abyme_model::{Board, Element, Id, IdKind};

use super::files::{
    MAX_FILE_BYTES, ensure_directory, ensure_local_dir, exists, io_error, read_limited,
};
use super::location::{BOARD_SUFFIX, ELEMENT_SUFFIX, Location};
use super::{EntryKind, ProjectError};
use crate::apply::{Created, Snapshot, Write, apply};
use crate::operation::{LocalKey, Operation};

/// Counts the temporary files of this process, so that their names never
/// repeat.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// One change to a file of the project, ready to carry out.
#[derive(Debug)]
enum FileChange<'id> {
    /// Write this text to the file.
    Write(Location<'id>, String),
    /// Remove the file.
    Remove(Location<'id>),
}

/// Reads every element and board file. A missing `abyme/elements/` or
/// `abyme/boards/` counts as empty.
pub fn read_snapshot(root: &Path) -> Result<Snapshot, ProjectError> {
    exists(root, Location::AbymeDir, EntryKind::Directory)?;
    let elements = list(root, Location::ElementsDir, ELEMENT_SUFFIX)?
        .into_iter()
        .map(|id| {
            let text = read_limited(root, Location::Element(&id), MAX_FILE_BYTES)?;
            let element = Element::from_markdown(&id, &text)?;
            Ok((id, element))
        })
        .collect::<Result<BTreeMap<_, _>, ProjectError>>()?;
    let boards = list(root, Location::BoardsDir, BOARD_SUFFIX)?
        .into_iter()
        .map(|id| {
            let text = read_limited(root, Location::Board(&id), MAX_FILE_BYTES)?;
            let board = Board::from_json(&id, &text)?;
            Ok((id, board))
        })
        .collect::<Result<BTreeMap<_, _>, ProjectError>>()?;
    Ok(Snapshot { elements, boards })
}

/// Applies `operations` while holding the lock, from reading the files to
/// the last rename, and returns the ID each key received.
pub fn apply_operations(
    root: &Path,
    operations: &[Operation],
) -> Result<Vec<(LocalKey, Created)>, ProjectError> {
    ensure_local_dir(root)?;
    let lock = open_lock(root)?;
    lock.lock().map_err(io_error(Location::Lock))?;
    let applied = apply(&read_snapshot(root)?, operations)?;
    let files = serialized(&applied.writes)?;
    if !files.is_empty() {
        write_files(root, &files)?;
    }
    Ok(applied.created)
}

/// The IDs of the regular files in the directory at `location` whose names
/// are an ID followed by `suffix`; every other entry is ignored.
fn list<K: IdKind>(
    root: &Path,
    location: Location,
    suffix: &str,
) -> Result<Vec<Id<K>>, ProjectError> {
    if !exists(root, location, EntryKind::Directory)? {
        return Ok(Vec::new());
    }
    fs::read_dir(root.join(location.relative()))
        .and_then(|entries| {
            entries
                .map(|entry| listed_id(entry, suffix))
                .filter_map(Result::transpose)
                .collect()
        })
        .map_err(io_error(location))
}

/// The ID an entry names, if it is a regular file named as an ID followed
/// by `suffix`.
fn listed_id<K: IdKind>(entry: io::Result<DirEntry>, suffix: &str) -> io::Result<Option<Id<K>>> {
    let entry = entry?;
    if !entry.file_type()?.is_file() {
        return Ok(None);
    }
    Ok(entry
        .file_name()
        .to_str()
        .and_then(|name| name.strip_suffix(suffix))
        .and_then(|stem| stem.parse().ok()))
}

/// Opens the lock file, creating it if it does not exist.
fn open_lock(root: &Path) -> Result<File, ProjectError> {
    exists(root, Location::Lock, EntryKind::File)?;
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(root.join(Location::Lock.relative()))
        .map_err(io_error(Location::Lock))
}

/// The content of every file to write and every file to remove, rejected
/// as a whole when a file to write would be larger than the reader accepts.
fn serialized(writes: &[Write]) -> Result<Vec<FileChange<'_>>, ProjectError> {
    writes
        .iter()
        .map(|write| {
            let (location, text) = match write {
                Write::Element(element) => (Location::Element(element.id()), element.to_markdown()),
                Write::Board(board) => (Location::Board(&board.id), board.to_json()),
                Write::RemoveElement(id) => return Ok(FileChange::Remove(Location::Element(id))),
            };
            if text.len() > MAX_FILE_BYTES {
                Err(ProjectError::WriteTooLarge {
                    path: location.relative(),
                    size: text.len(),
                    limit: MAX_FILE_BYTES,
                })
            } else {
                Ok(FileChange::Write(location, text))
            }
        })
        .collect()
}

/// Carries out every change, in order: each file to write through a
/// temporary file and a rename, each file to remove with `remove_file`.
/// Leftovers of a crashed writer are removed first; the caller holds the
/// lock, so no other writer's temporary file is among them.
fn write_files(root: &Path, files: &[FileChange]) -> Result<(), ProjectError> {
    ensure_directory(root, Location::TempDir)?;
    remove_leftovers(root)?;
    ensure_directory(root, Location::ElementsDir)?;
    ensure_directory(root, Location::BoardsDir)?;
    files.iter().try_for_each(|change| match change {
        FileChange::Write(location, text) => write_file(root, *location, text),
        FileChange::Remove(location) => remove_file(root, *location),
    })
}

/// Removes the file at `location`; on a symbolic link that is the link,
/// not its target. A file that is already gone counts as removed: the
/// request's boards are written by then, and the file is gone either way.
fn remove_file(root: &Path, location: Location) -> Result<(), ProjectError> {
    match fs::remove_file(root.join(location.relative())) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(location)(error)),
    }
}

/// Removes the files and symbolic links in `.abyme/tmp/`; directories stay.
fn remove_leftovers(root: &Path) -> Result<(), ProjectError> {
    let remove = || -> io::Result<()> {
        for entry in fs::read_dir(root.join(Location::TempDir.relative()))? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_file() || kind.is_symlink() {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    };
    remove().map_err(io_error(Location::TempDir))
}

/// Writes `text` to a new temporary file and renames it to `location`. The
/// rename replaces whatever entry is there without following it.
fn write_file(root: &Path, location: Location, text: &str) -> Result<(), ProjectError> {
    let temp = Location::Temp {
        process: process::id(),
        counter: TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
    };
    let temp_path = root.join(temp.relative());
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })
        .map_err(io_error(temp))?;
    fs::rename(&temp_path, root.join(location.relative())).map_err(io_error(location))
}

#[cfg(test)]
mod tests;
