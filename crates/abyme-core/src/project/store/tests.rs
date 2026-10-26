//! Tests of removing element files, which the public API reaches only for
//! regular files.

use std::os::unix::fs::symlink;

use abyme_model::ElementId;

use super::*;

/// A directory with `abyme/` and `.abyme/`, as a project has them.
fn root() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join("abyme/elements")).unwrap();
    fs::create_dir(directory.path().join(".abyme")).unwrap();
    directory
}

fn removal(id: &ElementId) -> [FileChange<'_>; 1] {
    [FileChange::Remove(Location::Element(id))]
}

fn id(value: &str) -> ElementId {
    value.parse().unwrap()
}

#[test]
fn removes_an_element_file() {
    let root = root();
    let file = root.path().join("abyme/elements/x.md");
    fs::write(&file, "x").unwrap();

    write_files(root.path(), &removal(&id("x"))).unwrap();

    assert!(!file.exists());
}

#[test]
fn counts_a_file_already_gone_as_removed() {
    let root = root();

    write_files(root.path(), &removal(&id("x"))).unwrap();
}

#[test]
fn removes_a_symbolic_link_and_leaves_its_target() {
    let root = root();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("kept.md");
    fs::write(&target, "kept").unwrap();
    let link = root.path().join("abyme/elements/x.md");
    symlink(&target, &link).unwrap();

    write_files(root.path(), &removal(&id("x"))).unwrap();

    assert!(fs::symlink_metadata(&link).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "kept");
}

#[test]
fn removes_no_directory_at_an_element_path() {
    let root = root();
    let directory = root.path().join("abyme/elements/x.md");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("inside"), "kept").unwrap();

    let error = write_files(root.path(), &removal(&id("x"))).unwrap_err();

    assert!(
        matches!(&error, ProjectError::Io { path, .. } if *path == Path::new("abyme/elements/x.md")),
        "{error:?}"
    );
    assert_eq!(
        fs::read_to_string(directory.join("inside")).unwrap(),
        "kept"
    );
}

#[test]
fn removes_nothing_through_an_elements_directory_that_is_a_link() {
    let root = root();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("x.md"), "kept").unwrap();
    fs::remove_dir(root.path().join("abyme/elements")).unwrap();
    symlink(outside.path(), root.path().join("abyme/elements")).unwrap();

    let error = write_files(root.path(), &removal(&id("x"))).unwrap_err();

    assert!(
        matches!(&error, ProjectError::WrongKind { path, .. } if *path == Path::new("abyme/elements")),
        "{error:?}"
    );
    assert_eq!(
        fs::read_to_string(outside.path().join("x.md")).unwrap(),
        "kept"
    );
}
