//! Review flags of a project in a real Git repository, through
//! `Project::review`.

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use abyme_core::{Flag, Project, Review, Unavailable};
use abyme_model::ElementId;
use rstest::rstest;
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// A stamp no repository of these tests holds.
const UNKNOWN: &str = "0123456789abcdef0123456789abcdef01234567";

/// Runs `git` in `directory` without the user's or the system's
/// configuration, and returns what it prints.
fn git(directory: &Path, arguments: &[&str]) -> TestResult<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args([
            "-c",
            "user.name=Abyme Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "tag.gpgSign=false",
        ])
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()?;
    if !output.status.success() {
        return Err(format!("git {arguments:?} failed: {output:?}").into());
    }
    Ok(String::from_utf8(output.stdout)?.trim_end().to_owned())
}

/// An Abyme project at the top of a new Git repository, or in the
/// subdirectory `project` of it.
struct Repository {
    directory: TempDir,
    project: PathBuf,
}

impl Repository {
    /// A repository whose first commit holds the project file.
    fn new() -> TestResult<Self> {
        Self::with_project(Path::new(""))
    }

    fn with_project(project: &Path) -> TestResult<Self> {
        let repository = Self::without_commit(project)?;
        repository.commit()?;
        Ok(repository)
    }

    fn without_commit(project: &Path) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        git(directory.path(), &["init", "-q"])?;
        let project = directory.path().join(project);
        fs::create_dir_all(&project)?;
        Project::init(&project)?;
        Ok(Self { directory, project })
    }

    fn top(&self) -> &Path {
        self.directory.path()
    }

    /// Writes `content` to `path`, relative to the project root.
    fn write(&self, path: &[u8], content: &str) -> TestResult {
        let path = self.project.join(OsStr::from_bytes(path));
        fs::create_dir_all(path.parent().ok_or("no parent")?)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Commits everything and returns the new HEAD.
    fn commit(&self) -> TestResult<String> {
        git(self.top(), &["add", "-A"])?;
        git(
            self.top(),
            &["commit", "-q", "--allow-empty", "-m", "change"],
        )?;
        git(self.top(), &["rev-parse", "HEAD"])
    }

    /// Writes the element `x` with the given anchor and stamp.
    fn element(&self, anchor: Option<&str>, stamp: Option<&str>) -> TestResult {
        let anchor = anchor.map_or(String::new(), |anchor| format!("anchor: \"{anchor}\"\n"));
        let stamp = stamp.map_or(String::new(), |stamp| {
            format!("verified: \"{stamp}\"\nverified_by: \"person\"\n")
        });
        self.write(
            b"abyme/elements/x.md",
            &format!("---\nid: \"x\"\ntype: \"t\"\nname: \"X\"\n{anchor}{stamp}---\n"),
        )
    }

    fn review(&self) -> TestResult<Result<Review, Unavailable>> {
        let project = Project::discover(&self.project)?;
        let snapshot = project.snapshot()?;
        Ok(project.review(&snapshot))
    }

    /// The flags of the element `x`.
    fn flags(&self) -> TestResult<Vec<Flag>> {
        let id: ElementId = "x".parse()?;
        Ok(self.review()??.flags(&id).to_vec())
    }
}

fn changed(paths: &[&[u8]]) -> Flag {
    Flag::Changed(paths.iter().map(|path| path.to_vec()).collect())
}

/// A repository whose element `x` is anchored on `crates/x`, holding
/// `lib.rs`, and stamped with the commit that added it; returns the stamp.
fn verified() -> TestResult<Repository> {
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    let stamp = repository.commit()?;
    repository.element(Some("crates/x"), Some(&stamp))?;
    repository.commit()?;
    Ok(repository)
}

#[test]
fn flags_nothing_while_nothing_under_the_anchor_changed() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/other.rs", "fn b() {}\n")?;
    repository.commit()?;

    assert_eq!(repository.flags()?, []);
    Ok(())
}

#[test]
fn flags_a_file_committed_since_the_stamp() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    repository.commit()?;

    assert_eq!(repository.flags()?, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}

#[test]
fn flags_a_staged_change() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    git(repository.top(), &["add", "-A"])?;

    assert_eq!(repository.flags()?, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}

#[test]
fn flags_a_change_in_the_working_tree_and_an_untracked_file() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    repository.write(b"crates/x/new.rs", "fn c() {}\n")?;

    assert_eq!(
        repository.flags()?,
        [changed(&[b"crates/x/lib.rs", b"crates/x/new.rs"])]
    );
    Ok(())
}

#[test]
fn flags_a_removed_file_and_then_the_missing_anchor() -> TestResult {
    let repository = verified()?;
    fs::remove_dir_all(repository.project.join("crates/x"))?;
    repository.commit()?;

    assert_eq!(
        repository.flags()?,
        [changed(&[b"crates/x/lib.rs"]), Flag::AnchorMissing]
    );
    Ok(())
}

#[test]
fn flags_an_anchored_element_without_a_stamp() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.element(Some("crates/x"), None)?;
    repository.commit()?;

    assert_eq!(repository.flags()?, [Flag::NeverVerified]);
    Ok(())
}

#[test]
fn flags_an_anchor_that_matches_no_file_and_ignores_ignored_files() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b".gitignore", "target\n")?;
    repository.write(b"target/debug/x", "built\n")?;
    let stamp = repository.commit()?;
    repository.element(Some("target"), Some(&stamp))?;

    assert_eq!(repository.flags()?, [Flag::AnchorMissing]);
    Ok(())
}

#[test]
fn does_not_count_a_sibling_with_the_same_start() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/xy/lib.rs", "fn d() {}\n")?;

    assert_eq!(repository.flags()?, []);
    Ok(())
}

/// Kinds of stamps that name no ancestor commit of HEAD.
#[derive(Debug, Clone, Copy)]
enum OtherStamp {
    Unknown,
    LongerHash,
    Tree,
    AnnotatedTag,
    OtherBranch,
}

/// A stamp of the given kind in `repository`.
fn other_stamp(repository: &Repository, kind: OtherStamp) -> TestResult<String> {
    match kind {
        OtherStamp::Unknown => Ok(UNKNOWN.to_owned()),
        OtherStamp::LongerHash => Ok("ab".repeat(32)),
        OtherStamp::Tree => git(repository.top(), &["rev-parse", "HEAD^{tree}"]),
        OtherStamp::AnnotatedTag => {
            git(repository.top(), &["tag", "-a", "-m", "tag", "v1"])?;
            git(repository.top(), &["rev-parse", "v1"])
        }
        OtherStamp::OtherBranch => {
            git(repository.top(), &["switch", "-q", "-c", "side"])?;
            repository.write(b"side.rs", "fn s() {}\n")?;
            let side = repository.commit()?;
            git(repository.top(), &["switch", "-q", "main"])?;
            Ok(side)
        }
    }
}

#[rstest]
#[case::unknown_commit(OtherStamp::Unknown)]
#[case::sha256_length_in_a_sha1_repository(OtherStamp::LongerHash)]
#[case::a_tree(OtherStamp::Tree)]
#[case::an_annotated_tag(OtherStamp::AnnotatedTag)]
#[case::a_commit_on_another_branch(OtherStamp::OtherBranch)]
fn flags_a_stamp_that_names_no_ancestor_commit(#[case] kind: OtherStamp) -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.commit()?;
    let stamp = other_stamp(&repository, kind)?;
    repository.element(Some("crates/x"), Some(&stamp))?;

    assert_eq!(repository.flags()?, [Flag::UnknownStamp]);
    Ok(())
}

#[test]
fn flags_a_stamp_in_a_repository_without_commits() -> TestResult {
    let repository = Repository::without_commit(Path::new(""))?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.element(Some("crates/x"), Some(UNKNOWN))?;

    assert_eq!(repository.flags()?, [Flag::UnknownStamp]);
    Ok(())
}

#[test]
fn reads_a_project_below_the_top_of_its_repository() -> TestResult {
    let repository = Repository::with_project(Path::new("pro ject\t\"ü"))?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    let stamp = repository.commit()?;
    repository.element(Some("crates/x"), Some(&stamp))?;
    fs::create_dir_all(repository.top().join("crates/x"))?;
    fs::write(repository.top().join("crates/x/outside.rs"), "fn o() {}\n")?;
    repository.commit()?;
    repository.write(b"crates/x/new.rs", "fn n() {}\n")?;

    assert_eq!(repository.flags()?, [changed(&[b"crates/x/new.rs"])]);
    Ok(())
}

#[test]
fn reads_file_names_git_would_quote() -> TestResult {
    let repository = verified()?;
    let names: [&[u8]; 5] = [
        b"crates/x/a\nb",
        b"crates/x/tab\there",
        b"crates/x/-leading",
        b"crates/x/\xff",
        b"crates/x/\xe2\x80\xaeevil",
    ];
    for name in names {
        repository.write(name, "x\n")?;
    }

    let mut expected = names.to_vec();
    expected.sort_unstable();
    assert_eq!(repository.flags()?, [changed(&expected)]);
    Ok(())
}

#[test]
fn never_hands_an_anchor_to_git() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"--output=evil/a", "x\n")?;
    let stamp = repository.commit()?;
    repository.element(Some("--output=evil"), Some(&stamp))?;
    repository.write(b"--output=evil/a", "y\n")?;

    assert_eq!(repository.flags()?, [changed(&[b"--output=evil/a"])]);
    assert!(!repository.project.join("evil").exists());
    Ok(())
}

#[test]
fn leaves_the_index_as_it_is() -> TestResult {
    let repository = verified()?;
    fs::File::options()
        .write(true)
        .open(repository.project.join("crates/x/lib.rs"))?
        .set_modified(SystemTime::now() + Duration::from_secs(60))?;
    let index = repository.top().join(".git/index");
    let before = (fs::read(&index)?, fs::metadata(&index)?.modified()?);

    let flags = repository.flags()?;

    assert_eq!(flags, []);
    assert_eq!(
        (fs::read(&index)?, fs::metadata(&index)?.modified()?),
        before
    );
    assert!(!repository.top().join(".git/index.lock").exists());
    Ok(())
}

#[test]
fn reads_while_another_git_command_holds_the_index_lock() -> TestResult {
    let repository = verified()?;
    fs::write(repository.top().join(".git/index.lock"), "")?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;

    assert_eq!(repository.flags()?, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}

#[test]
fn reads_a_repository_whose_configuration_shows_stashes() -> TestResult {
    let repository = verified()?;
    git(repository.top(), &["config", "status.showStash", "true"])?;
    repository.write(b"crates/x/lib.rs", "fn a() { stashed() }\n")?;
    git(repository.top(), &["stash", "-q"])?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;

    assert_eq!(repository.flags()?, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}

/// A cloned file layout that Git would take for a bare repository: its
/// `config` runs a clean filter that leaves `marker` behind.
fn embedded_bare_repository(directory: &Path, marker: &Path, project: &Path) -> TestResult {
    git(directory, &["init", "--bare", "-q", "."])?;
    git(directory, &["config", "core.bare", "false"])?;
    git(directory, &["config", "core.worktree", "."])?;
    let project = directory.join(project);
    fs::create_dir_all(&project)?;
    Project::init(&project)?;
    fs::write(directory.join(".gitattributes"), "* filter=x\n")?;
    fs::write(directory.join("file.txt"), "content\n")?;
    git(directory, &["add", ".gitattributes", "file.txt", "."])?;
    git(directory, &["commit", "-q", "-m", "files"])?;
    let filter = format!("sh -c 'touch {}; cat'", marker.display());
    git(directory, &["config", "filter.x.clean", &filter])?;
    fs::File::options()
        .write(true)
        .open(directory.join("file.txt"))?
        .set_modified(SystemTime::now() + Duration::from_secs(60))?;
    Ok(())
}

#[rstest]
#[case::at_its_top("")]
#[case::below_its_top("sub")]
fn never_uses_a_bare_repository_that_a_clone_could_bring(#[case] project: &str) -> TestResult {
    let directory = tempfile::tempdir()?;
    let bare = directory.path().join("bare");
    fs::create_dir(&bare)?;
    let marker = directory.path().join("marker");
    embedded_bare_repository(&bare, &marker, Path::new(project))?;
    let project = Project::discover(&bare.join(project))?;

    let review = project.review(&project.snapshot()?);

    assert_eq!(review, Err(Unavailable::NoRepository));
    assert!(!marker.exists());
    Ok(())
}

#[test]
fn is_unavailable_outside_a_repository() -> TestResult {
    let directory = tempfile::tempdir()?;
    Project::init(directory.path())?;
    let project = Project::discover(directory.path())?;

    let review = project.review(&project.snapshot()?);

    assert_eq!(review, Err(Unavailable::NoRepository));
    Ok(())
}

#[test]
fn is_unavailable_when_git_fails() -> TestResult {
    let repository = verified()?;
    fs::write(repository.top().join(".git/index"), "not an index")?;

    assert_eq!(repository.review()?, Err(Unavailable::GitFailed));
    Ok(())
}
