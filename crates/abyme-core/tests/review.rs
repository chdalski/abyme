//! Review flags and verification of a project in a real Git repository,
//! through `Project::review` and `Project::verify`.

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use abyme_core::{Flag, Priority, Project, Review, Unavailable};
use abyme_model::{ElementId, RelationKind, Verifier};
use abyme_test_support::{commit_all, git};
use rstest::rstest;
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// A stamp no repository of these tests holds.
const UNKNOWN: &str = "0123456789abcdef0123456789abcdef01234567";

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
        commit_all(self.top())
    }

    /// Writes the element `x` with the given anchor and stamp.
    fn element(&self, anchor: Option<&str>, stamp: Option<&str>) -> TestResult {
        self.node("x", anchor, stamp, &[])
    }

    /// Writes the element `id` with the given anchor and stamp, and a
    /// `depends_on` relation to each of `targets`.
    fn node(
        &self,
        id: &str,
        anchor: Option<&str>,
        stamp: Option<&str>,
        targets: &[&str],
    ) -> TestResult {
        let anchor = anchor.map_or(String::new(), |anchor| format!("anchor: \"{anchor}\"\n"));
        let stamp = stamp.map_or(String::new(), |stamp| {
            format!("verified: \"{stamp}\"\nverified_by: \"person\"\n")
        });
        let relations: Vec<String> = targets
            .iter()
            .map(|target| format!("  - {{kind: \"depends_on\", to: \"{target}\"}}\n"))
            .collect();
        let relations = if relations.is_empty() {
            String::new()
        } else {
            format!("relations:\n{}", relations.concat())
        };
        self.write(
            format!("abyme/elements/{id}.md").as_bytes(),
            &format!(
                "---\nid: \"{id}\"\ntype: \"t\"\nname: \"X\"\n{anchor}{stamp}{relations}---\n"
            ),
        )
    }

    fn review(&self) -> TestResult<Result<Review, Unavailable>> {
        let project = Project::discover(&self.project)?;
        let snapshot = project.snapshot()?;
        Ok(project.review(&snapshot))
    }

    /// Verifies the element `id` by the AI: the commit, or the error text.
    fn verify(&self, id: &str) -> TestResult<Result<String, String>> {
        let project = Project::discover(&self.project)?;
        Ok(project
            .verify(&id.parse()?, Verifier::Ai)
            .map(|commit| commit.to_string())
            .map_err(|error| error.to_string()))
    }

    /// The text of the element file of `id`.
    fn element_file(&self, id: &str) -> TestResult<String> {
        Ok(fs::read_to_string(
            self.project.join(format!("abyme/elements/{id}.md")),
        )?)
    }

    /// The flags of the element `x`.
    fn flags(&self) -> TestResult<Vec<Flag>> {
        self.flags_of("x")
    }

    /// The flags of the element `id`.
    fn flags_of(&self, id: &str) -> TestResult<Vec<Flag>> {
        let id: ElementId = id.parse()?;
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
    commit_all(directory)?;
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

/// The refusal for files under the anchor of `x` that are not committed.
fn uncommitted() -> Result<String, String> {
    Err(
        "element x cannot be verified: files under its anchor differ from HEAD \
         or are untracked; they must be committed first"
            .to_owned(),
    )
}

#[test]
fn verify_stamps_head_by_ai_and_changes_nothing_else() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    let head = repository.commit()?;
    let before = repository.element_file("x")?;
    assert_eq!(repository.flags()?, [changed(&[b"crates/x/lib.rs"])]);

    let verified = repository.verify("x")?;

    assert_eq!(verified?, head);
    let after = repository.element_file("x")?;
    let without_stamp = |text: &str| -> String {
        text.lines()
            .filter(|line| !line.starts_with("verified") && !line.starts_with("  - {path: "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(without_stamp(&after), without_stamp(&before));
    assert!(
        after.contains(&format!("verified: \"{head}\"\nverified_by: \"ai\"\n")),
        "{after}"
    );
    assert_eq!(repository.flags()?, []);
    repository.write(b"crates/x/lib.rs", "fn a() { c() }\n")?;
    assert_eq!(repository.flags()?, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}

#[test]
fn verify_records_the_content_of_the_anchor_and_of_each_target() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.write(b"crates/y/lib.rs", "fn b() {}\n")?;
    repository.node("x", Some("crates/x"), None, &["y", "z", "w"])?;
    repository.node("y", Some("crates/y"), None, &[])?;
    repository.node("z", Some("crates/z"), None, &[])?;
    repository.node("w", None, None, &[])?;
    let head = repository.commit()?;
    let object = |path: &str| git(repository.top(), &["rev-parse", &format!("HEAD:{path}")]);

    let verified = repository.verify("x")?;

    assert_eq!(verified, Ok(head));
    let expected = format!(
        "verified_content:\n  - {{path: \"crates/x\", object: \"{}\"}}\n  \
         - {{path: \"crates/y\", object: \"{}\"}}\n  - {{path: \"crates/z\"}}\n",
        object("crates/x")?,
        object("crates/y")?,
    );
    let file = repository.element_file("x")?;
    assert!(file.contains(&expected), "{file}");
    Ok(())
}

#[test]
fn verify_names_the_commit_of_a_detached_head() -> TestResult {
    let repository = verified()?;
    git(repository.top(), &["switch", "-q", "--detach"])?;
    let head = git(repository.top(), &["rev-parse", "HEAD"])?;

    assert_eq!(repository.verify("x")??, head);
    Ok(())
}

#[test]
fn verify_refuses_a_change_in_the_working_tree() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    let before = repository.element_file("x")?;

    assert_eq!(repository.verify("x")?, uncommitted());
    assert_eq!(repository.element_file("x")?, before);
    Ok(())
}

#[test]
fn verify_refuses_a_staged_change() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    git(repository.top(), &["add", "-A"])?;

    assert_eq!(repository.verify("x")?, uncommitted());
    Ok(())
}

#[test]
fn verify_refuses_an_untracked_file() -> TestResult {
    let repository = verified()?;
    repository.write(b"crates/x/new.rs", "fn n() {}\n")?;

    assert_eq!(repository.verify("x")?, uncommitted());
    Ok(())
}

#[rstest]
#[case::in_the_working_tree(false)]
#[case::staged(true)]
fn verify_refuses_a_removed_file(#[case] staged: bool) -> TestResult {
    let repository = verified()?;
    fs::remove_file(repository.project.join("crates/x/lib.rs"))?;
    if staged {
        git(repository.top(), &["add", "-A"])?;
    }

    assert_eq!(repository.verify("x")?, uncommitted());
    Ok(())
}

#[test]
fn verify_ignores_changes_outside_the_anchor_and_ignored_files() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b".gitignore", "*.log\n")?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.element(Some("crates/x"), None)?;
    repository.commit()?;
    repository.write(b"crates/xy/lib.rs", "fn d() {}\n")?;
    repository.write(b"crates/x/debug.log", "noise\n")?;

    assert!(repository.verify("x")?.is_ok());
    Ok(())
}

#[test]
fn verify_keeps_the_flag_of_an_anchor_that_matches_no_file() -> TestResult {
    let repository = Repository::new()?;
    repository.element(Some("crates/gone"), None)?;
    repository.commit()?;

    assert!(repository.verify("x")?.is_ok());
    assert_eq!(repository.flags()?, [Flag::AnchorMissing]);
    Ok(())
}

#[test]
fn verify_measures_passed_on_flags_from_the_new_stamp() -> TestResult {
    let repository = verified()?;
    let old = repository.commit()?;
    repository.write(
        b"abyme/elements/s.md",
        &format!(
            "---\nid: s\ntype: t\nname: S\nverified: \"{old}\"\nverified_by: person\n\
             relations:\n  - {{kind: depends_on, to: x}}\n---\n"
        ),
    )?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    repository.commit()?;
    let project = Project::discover(&repository.project)?;
    let review = project.review(&project.snapshot()?)?;
    assert_eq!(review.flags(&"s".parse()?).len(), 1);

    assert!(repository.verify("s")?.is_ok());

    let review = project.review(&project.snapshot()?)?;
    assert_eq!(review.flags(&"s".parse()?), []);
    assert!(
        repository
            .element_file("s")?
            .contains("verified_by: \"ai\"")
    );
    Ok(())
}

#[test]
fn verify_refuses_without_a_commit_a_file_or_a_repository() -> TestResult {
    let unborn = Repository::without_commit(Path::new(""))?;
    unborn.element(None, None)?;
    let committed = verified()?;
    let outside = tempfile::tempdir()?;
    Project::init(outside.path())?;
    fs::create_dir_all(outside.path().join("abyme/elements"))?;
    fs::write(
        outside.path().join("abyme/elements/x.md"),
        "---\nid: x\ntype: t\nname: X\n---\n",
    )?;
    let x: ElementId = "x".parse()?;

    let refusals = [
        unborn.verify("x")?,
        committed.verify("y")?,
        Project::discover(outside.path())?
            .verify(&x, Verifier::Ai)
            .map(|commit| commit.to_string())
            .map_err(|error| error.to_string()),
    ];

    assert_eq!(
        refusals,
        [
            Err("element x cannot be verified: the repository has no commit yet".to_owned()),
            Err("no element has the ID y".to_owned()),
            Err("element x cannot be verified: the project lies in no Git repository".to_owned()),
        ]
    );
    Ok(())
}

/// How a repository is nested in the project.
#[derive(Debug, Clone, Copy)]
enum Nesting {
    /// Added with `git submodule add`; its Git directory lies in the outer
    /// repository's store of submodules.
    Submodule,
    /// A clone the outer repository tracks as a gitlink.
    Embedded,
    /// A clone the outer repository does not track.
    Untracked,
}

/// A repository at `directory` whose one commit holds `a.rs` and `b.rs`;
/// returns that commit.
fn library(directory: &Path) -> TestResult<String> {
    fs::create_dir_all(directory)?;
    git(directory, &["init", "-q"])?;
    fs::write(directory.join("a.rs"), "fn a() {}\n")?;
    fs::write(directory.join("b.rs"), "fn b() {}\n")?;
    commit_all(directory)
}

impl Repository {
    /// Nests a clone of the repository at `source` at `path` of the project
    /// as `nesting` says, and commits the outer repository unless the clone
    /// stays untracked; a later `commit` would track an untracked clone.
    /// Returns the nested repository's directory.
    fn nest(&self, source: &Path, path: &str, nesting: Nesting) -> TestResult<PathBuf> {
        let path = self.project.strip_prefix(self.top())?.join(path);
        let path = path.to_str().ok_or("not UTF-8")?;
        match nesting {
            Nesting::Submodule => submodule_add(self.top(), source, path)?,
            Nesting::Embedded | Nesting::Untracked => {
                git(
                    self.top(),
                    &["clone", "-q", source.to_str().ok_or("not UTF-8")?, path],
                )?;
            }
        }
        if !matches!(nesting, Nesting::Untracked) {
            commit_all(self.top())?;
        }
        Ok(self.top().join(path))
    }
}

#[rstest]
#[case::submodule(Nesting::Submodule)]
#[case::embedded_clone(Nesting::Embedded)]
#[case::untracked_clone(Nesting::Untracked)]
fn flags_a_change_inside_a_nested_repository_measured_there(
    #[case] nesting: Nesting,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    let stamp = library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", nesting)?;
    repository.element(Some("vendor/lib"), Some(&stamp))?;
    let unchanged = repository.flags()?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;
    fs::write(nested.join("new.rs"), "fn n() {}\n")?;
    let in_the_working_tree = repository.flags()?;
    commit_all(&nested)?;

    let committed = repository.flags()?;

    assert_eq!(unchanged, []);
    let expected = [changed(&[b"vendor/lib/a.rs", b"vendor/lib/new.rs"])];
    assert_eq!(in_the_working_tree, expected);
    assert_eq!(committed, expected);
    Ok(())
}

#[rstest]
#[case::submodule(Nesting::Submodule)]
#[case::embedded_clone(Nesting::Embedded)]
#[case::untracked_clone(Nesting::Untracked)]
fn flags_a_stamp_the_nested_repository_does_not_know(#[case] nesting: Nesting) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    repository.nest(&sources.path().join("lib"), "vendor/lib", nesting)?;
    let outer = git(repository.top(), &["rev-parse", "HEAD"])?;
    repository.element(Some("vendor/lib/a.rs"), Some(&outer))?;

    assert_eq!(repository.flags()?, [Flag::UnknownStamp]);
    Ok(())
}

#[rstest]
#[case::submodule(Nesting::Submodule)]
#[case::embedded_clone(Nesting::Embedded)]
fn judges_an_anchor_that_contains_a_nested_repository_by_the_nested_root(
    #[case] nesting: Nesting,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", nesting)?;
    repository.write(b"vendor/README.md", "Vendored code.\n")?;
    let stamp = repository.commit()?;
    repository.element(Some("vendor"), Some(&stamp))?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;
    fs::write(nested.join("new.rs"), "fn n() {}\n")?;
    let with_dirty_files = repository.flags()?;
    commit_all(&nested)?;

    let with_a_moved_root = repository.flags()?;

    assert_eq!(with_dirty_files, []);
    assert_eq!(with_a_moved_root, [changed(&[b"vendor/lib"])]);
    Ok(())
}

#[test]
fn an_untracked_clone_neither_flags_the_anchor_that_contains_it_nor_empties_it() -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let stamp = git(repository.top(), &["rev-parse", "HEAD"])?;
    let nested = repository.nest(
        &sources.path().join("lib"),
        "vendor/clone",
        Nesting::Untracked,
    )?;
    repository.element(Some("vendor"), Some(&stamp))?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;

    assert_eq!(repository.flags()?, []);
    Ok(())
}

#[rstest]
#[case::submodule(Nesting::Submodule)]
#[case::untracked_clone(Nesting::Untracked)]
fn reads_a_nested_repository_of_a_project_below_the_top_of_its_repository(
    #[case] nesting: Nesting,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    let stamp = library(&sources.path().join("lib"))?;
    let repository = Repository::with_project(Path::new("sub"))?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", nesting)?;
    repository.element(Some("vendor/lib"), Some(&stamp))?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;

    let flags = repository.flags()?;

    assert_eq!(nested, repository.project.join("vendor/lib"));
    assert_eq!(flags, [changed(&[b"vendor/lib/a.rs"])]);
    Ok(())
}

#[test]
fn verify_is_not_refused_by_an_untracked_clone_under_the_anchor() -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    repository.write(b"vendor/README.md", "Vendored code.\n")?;
    repository.element(Some("vendor"), None)?;
    let head = repository.commit()?;
    let nested = repository.nest(
        &sources.path().join("lib"),
        "vendor/clone",
        Nesting::Untracked,
    )?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;

    assert_eq!(repository.verify("x")?, Ok(head));
    Ok(())
}

/// Adds a clone of the repository at `source` as a submodule at `path` of
/// the repository at `outer`, which allows Git to clone from a local path.
fn submodule_add(outer: &Path, source: &Path, path: &str) -> TestResult {
    git(
        outer,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            source.to_str().ok_or("not UTF-8")?,
            path,
        ],
    )?;
    Ok(())
}

/// A repository whose project holds a submodule at `vendor/lib` that itself
/// holds a submodule `inner`, both cloned from repositories in `sources`
/// and checked out; returns it with the commit `inner` names.
fn two_levels_deep(sources: &Path) -> TestResult<(Repository, String)> {
    let inner = sources.join("inner");
    let stamp = library(&inner)?;
    let middle = sources.join("middle");
    library(&middle)?;
    submodule_add(&middle, &inner, "inner")?;
    git(&middle, &["commit", "-q", "-m", "inner"])?;
    let repository = Repository::new()?;
    repository.nest(&middle, "vendor/lib", Nesting::Submodule)?;
    git(
        repository.top(),
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "-q",
            "--init",
            "--recursive",
        ],
    )?;
    Ok((repository, stamp))
}

#[test]
fn reads_a_repository_nested_two_levels_deep() -> TestResult {
    let sources = tempfile::tempdir()?;
    let (repository, stamp) = two_levels_deep(sources.path())?;
    repository.element(Some("vendor/lib/inner"), Some(&stamp))?;
    fs::write(
        repository.project.join("vendor/lib/inner/a.rs"),
        "fn a() { b() }\n",
    )?;

    let flags = repository.flags()?;

    assert!(
        repository
            .top()
            .join(".git/modules/vendor/lib/modules/inner")
            .is_dir()
    );
    assert_eq!(flags, [changed(&[b"vendor/lib/inner/a.rs"])]);
    Ok(())
}

#[test]
fn passes_flags_on_within_a_nested_repository_and_across_repositories() -> TestResult {
    let sources = tempfile::tempdir()?;
    let stamp = library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(
        &sources.path().join("lib"),
        "vendor/lib",
        Nesting::Submodule,
    )?;
    let outer = git(repository.top(), &["rev-parse", "HEAD"])?;
    repository.node("t", Some("vendor/lib/a.rs"), Some(&stamp), &[])?;
    repository.node("inside", Some("vendor/lib/b.rs"), Some(&stamp), &["t"])?;
    repository.node("outside", None, Some(&outer), &["t"])?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;
    commit_all(&nested)?;

    let review = repository.review()??;

    let upstream = [Flag::Upstream {
        kind: RelationKind::DependsOn,
        target: "t".parse()?,
        priority: Priority::Task,
    }];
    assert_eq!(
        review.flags(&"t".parse()?),
        [changed(&[b"vendor/lib/a.rs"])]
    );
    assert_eq!(review.flags(&"inside".parse()?), upstream);
    assert_eq!(review.flags(&"outside".parse()?), upstream);
    Ok(())
}

#[test]
fn an_anchor_inside_a_clone_that_git_ignores_matches_no_file() -> TestResult {
    let sources = tempfile::tempdir()?;
    let stamp = library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    repository.write(b".gitignore", "vendor/\n")?;
    repository.commit()?;
    repository.nest(
        &sources.path().join("lib"),
        "vendor/lib",
        Nesting::Untracked,
    )?;
    repository.element(Some("vendor/lib/a.rs"), Some(&stamp))?;

    assert_eq!(
        repository.flags()?,
        [Flag::UnknownStamp, Flag::AnchorMissing]
    );
    Ok(())
}

/// A nested repository that Git must not run in, because it lies elsewhere
/// than where the project's Git output names it, or is no repository.
#[derive(Debug, Clone, Copy)]
enum Elsewhere {
    /// A submodule whose directory is empty.
    UninitializedSubmodule,
    /// A gitlink whose directory holds files but no `.git`.
    GitlinkWithoutGit,
    /// A `.git` file whose `gitdir:` points outside the project.
    GitFileOutside,
    /// A submodule whose Git directory names a common directory outside.
    CommonDirectoryOutside,
    /// A `.git` that is a symbolic link to a Git directory outside.
    GitSymlink,
    /// A clone whose `core.worktree` is the directory above it.
    WorkTreeAbove,
    /// A clone whose `core.worktree` lies outside the project.
    WorkTreeOutside,
    /// A linked work tree of a repository outside the project.
    LinkedWorkTree,
    /// A gitlink whose directory is a symbolic link to a repository.
    SymlinkedRepository,
    /// A gitlink below a directory that is a symbolic link.
    SymlinkedParent,
    /// A repository in a submodule whose `gitdir:` points outside both the
    /// project and the submodule's store.
    SecondLevelOutside,
}

/// A repository outside the project with `a.rs`, a committed
/// `outside-secret.rs`, and an untracked `outside-untracked.rs`.
fn outside(directory: &Path) -> TestResult {
    library(directory)?;
    fs::write(directory.join("outside-secret.rs"), "fn s() {}\n")?;
    commit_all(directory)?;
    fs::write(directory.join("outside-untracked.rs"), "fn u() {}\n")?;
    Ok(())
}

/// Sets up the nested repository of `case` in `repository`, with `lib` and
/// the repository `outside` at hand; returns the anchor to test.
fn nest_elsewhere(
    repository: &Repository,
    lib: &Path,
    outside: &Path,
    case: Elsewhere,
) -> TestResult<&'static str> {
    let top = repository.top();
    let gitdir = |directory: &Path| -> TestResult {
        fs::create_dir_all(directory)?;
        fs::write(directory.join("a.rs"), "fn a() {}\n")?;
        fs::write(
            directory.join(".git"),
            format!("gitdir: {}\n", outside.join(".git").display()),
        )?;
        Ok(())
    };
    match case {
        Elsewhere::UninitializedSubmodule => {
            repository.nest(lib, "vendor/lib", Nesting::Submodule)?;
            git(top, &["submodule", "deinit", "-q", "-f", "vendor/lib"])?;
        }
        Elsewhere::GitlinkWithoutGit => {
            let nested = repository.nest(lib, "vendor/lib", Nesting::Embedded)?;
            fs::remove_dir_all(nested.join(".git"))?;
        }
        Elsewhere::GitFileOutside => gitdir(&top.join("vendor/lib"))?,
        Elsewhere::CommonDirectoryOutside => {
            repository.nest(lib, "vendor/lib", Nesting::Submodule)?;
            fs::write(
                top.join(".git/modules/vendor/lib/commondir"),
                format!("{}\n", outside.join(".git").display()),
            )?;
        }
        Elsewhere::GitSymlink => {
            let nested = top.join("vendor/lib");
            fs::create_dir_all(&nested)?;
            fs::write(nested.join("a.rs"), "fn a() {}\n")?;
            symlink(outside.join(".git"), nested.join(".git"))?;
        }
        Elsewhere::WorkTreeAbove => {
            let nested = repository.nest(lib, "vendor/lib", Nesting::Untracked)?;
            git(&nested, &["config", "core.worktree", "../.."])?;
        }
        Elsewhere::WorkTreeOutside => {
            let nested = repository.nest(lib, "vendor/lib", Nesting::Untracked)?;
            git(
                &nested,
                &[
                    "config",
                    "core.worktree",
                    outside.to_str().ok_or("not UTF-8")?,
                ],
            )?;
        }
        Elsewhere::LinkedWorkTree => {
            fs::create_dir_all(top.join("vendor"))?;
            git(
                outside,
                &[
                    "worktree",
                    "add",
                    "-q",
                    top.join("vendor/lib").to_str().ok_or("not UTF-8")?,
                ],
            )?;
        }
        Elsewhere::SymlinkedRepository => {
            let nested = repository.nest(lib, "vendor/lib", Nesting::Embedded)?;
            fs::remove_dir_all(&nested)?;
            symlink(outside, &nested)?;
        }
        Elsewhere::SymlinkedParent => {
            repository.nest(lib, "vendor/lib", Nesting::Embedded)?;
            let moved = outside.with_file_name("moved-vendor");
            fs::rename(top.join("vendor"), &moved)?;
            symlink(&moved, top.join("vendor"))?;
        }
        Elsewhere::SecondLevelOutside => {
            let nested = repository.nest(lib, "vendor/lib", Nesting::Submodule)?;
            gitdir(&nested.join("inner"))?;
            return Ok("vendor/lib/inner/a.rs");
        }
    }
    Ok("vendor/lib/a.rs")
}

#[rstest]
#[case::uninitialized_submodule(Elsewhere::UninitializedSubmodule)]
#[case::gitlink_without_git(Elsewhere::GitlinkWithoutGit)]
#[case::git_file_outside(Elsewhere::GitFileOutside)]
#[case::common_directory_outside(Elsewhere::CommonDirectoryOutside)]
#[case::git_symlink(Elsewhere::GitSymlink)]
#[case::work_tree_above(Elsewhere::WorkTreeAbove)]
#[case::work_tree_outside(Elsewhere::WorkTreeOutside)]
#[case::linked_work_tree(Elsewhere::LinkedWorkTree)]
#[case::symlinked_repository(Elsewhere::SymlinkedRepository)]
#[case::symlinked_parent(Elsewhere::SymlinkedParent)]
#[case::second_level_outside(Elsewhere::SecondLevelOutside)]
fn never_runs_git_in_a_nested_repository_that_lies_elsewhere(
    #[case] case: Elsewhere,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    let lib = sources.path().join("lib");
    library(&lib)?;
    let outside_repository = sources.path().join("outside");
    outside(&outside_repository)?;
    let repository = Repository::new()?;
    let anchor = nest_elsewhere(&repository, &lib, &outside_repository, case)?;
    repository.element(Some(anchor), None)?;

    let flags = repository.flags()?;

    assert_eq!(flags, [Flag::NeverVerified, Flag::AnchorMissing]);
    Ok(())
}

/// A setting in a nested repository's own configuration that would run a
/// command.
#[derive(Debug, Clone, Copy)]
enum Hostile {
    /// A clean filter.
    Clean,
    /// A long-running process filter.
    Process,
    /// A clean filter whose driver name holds `.`, `=`, and capitals.
    OddDriverName,
    /// A clean filter in a file that the configuration includes.
    Included,
    /// A clean filter in the work tree's own configuration.
    WorkTreeConfig,
    /// A clean filter that must succeed.
    Required,
    /// A clean filter named in `.git/info/attributes`.
    InfoAttributes,
    /// A file system monitor.
    Fsmonitor,
    /// A hook that runs when the index changes.
    Hook,
}

/// Sets up `case` in the nested repository at `nested`, with commands that
/// leave `marker` behind; `scripts` is a directory outside the project.
fn make_hostile(nested: &Path, case: Hostile, marker: &Path, scripts: &Path) -> TestResult {
    let touch = format!("sh -c 'touch {}; cat'", marker.display());
    let script = scripts.join("touch.sh");
    fs::write(&script, format!("#!/bin/sh\ntouch {}\n", marker.display()))?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
    let driver = if matches!(case, Hostile::OddDriverName) {
        "Evil.a=b"
    } else {
        "x"
    };
    let attributes = format!("* filter={driver}\n");
    if matches!(case, Hostile::InfoAttributes) {
        fs::write(nested.join(".git/info/attributes"), &attributes)?;
    } else {
        fs::write(nested.join(".gitattributes"), &attributes)?;
        commit_all(nested)?;
    }
    let config = |arguments: &[&str]| git(nested, &[&["config"], arguments].concat()).map(drop);
    match case {
        Hostile::Clean | Hostile::InfoAttributes => config(&["filter.x.clean", &touch])?,
        Hostile::Process => config(&["filter.x.process", &touch])?,
        Hostile::OddDriverName => config(&["filter.Evil.a=b.clean", &touch])?,
        Hostile::Included => {
            let included = scripts.join("included.cfg");
            fs::write(
                &included,
                format!("[filter \"x\"]\n\tclean = \"{touch}\"\n"),
            )?;
            config(&["include.path", included.to_str().ok_or("not UTF-8")?])?;
        }
        Hostile::WorkTreeConfig => {
            config(&["core.repositoryFormatVersion", "1"])?;
            config(&["extensions.worktreeConfig", "true"])?;
            config(&["--worktree", "filter.x.clean", &touch])?;
        }
        Hostile::Required => {
            config(&["filter.x.clean", &touch])?;
            config(&["filter.x.required", "true"])?;
        }
        Hostile::Fsmonitor => config(&["core.fsmonitor", script.to_str().ok_or("not UTF-8")?])?,
        Hostile::Hook => {
            let hooks = scripts.join("hooks");
            fs::create_dir(&hooks)?;
            fs::copy(&script, hooks.join("post-index-change"))?;
            config(&["core.hooksPath", hooks.to_str().ok_or("not UTF-8")?])?;
        }
    }
    fs::File::options()
        .write(true)
        .open(nested.join("a.rs"))?
        .set_modified(SystemTime::now() + Duration::from_secs(60))?;
    Ok(())
}

#[rstest]
#[case::clean_filter(Hostile::Clean)]
#[case::process_filter(Hostile::Process)]
#[case::odd_driver_name(Hostile::OddDriverName)]
#[case::included_filter(Hostile::Included)]
#[case::work_tree_configuration(Hostile::WorkTreeConfig)]
#[case::required_filter(Hostile::Required)]
#[case::info_attributes(Hostile::InfoAttributes)]
#[case::file_system_monitor(Hostile::Fsmonitor)]
#[case::index_hook(Hostile::Hook)]
fn runs_nothing_that_a_nested_repository_s_own_configuration_sets_up(
    #[case] case: Hostile,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", Nesting::Embedded)?;
    let marker = sources.path().join("marker");
    make_hostile(&nested, case, &marker, sources.path())?;
    let stamp = git(&nested, &["rev-parse", "HEAD"])?;
    repository.element(Some("vendor/lib"), Some(&stamp))?;

    let flags = repository.flags()?;

    assert!(!marker.exists());
    assert_eq!(flags, []);
    Ok(())
}

/// Clones a partial clone without trees of a two-commit repository to
/// `vendor/lib` of `repository`, whose remote is an SSH URL with a command
/// that leaves `marker` behind; returns its first commit and its HEAD.
fn partial_clone(
    repository: &Repository,
    sources: &Path,
    marker: &Path,
) -> TestResult<(String, String)> {
    let source = sources.join("source");
    let first = library(&source)?;
    fs::write(source.join("a.rs"), "fn a() { b() }\n")?;
    commit_all(&source)?;
    git(&source, &["config", "uploadpack.allowFilter", "true"])?;
    git(
        repository.top(),
        &[
            "clone",
            "-q",
            "--filter=tree:0",
            &format!("file://{}", source.display()),
            "vendor/lib",
        ],
    )?;
    let nested = repository.project.join("vendor/lib");
    git(
        &nested,
        &["remote", "set-url", "origin", "ssh://example.invalid/lib"],
    )?;
    git(
        &nested,
        &[
            "config",
            "core.sshCommand",
            &format!("sh -c 'touch {}'", marker.display()),
        ],
    )?;
    git(&nested, &["config", "protocol.ssh.allow", "always"])?;
    let head = git(&nested, &["rev-parse", "HEAD"])?;
    Ok((first, head))
}

#[test]
fn fetches_nothing_for_a_partial_clone() -> TestResult {
    let sources = tempfile::tempdir()?;
    let repository = Repository::new()?;
    let marker = sources.path().join("marker");
    let (first, _) = partial_clone(&repository, sources.path(), &marker)?;
    repository.element(Some("vendor/lib/a.rs"), Some(&first))?;

    let flags = repository.flags()?;

    assert!(!marker.exists());
    assert_eq!(flags, [Flag::UnknownStamp, Flag::AnchorMissing]);
    Ok(())
}

#[test]
fn verify_fetches_nothing_for_a_partial_clone() -> TestResult {
    let sources = tempfile::tempdir()?;
    let repository = Repository::new()?;
    let marker = sources.path().join("marker");
    let (_, head) = partial_clone(&repository, sources.path(), &marker)?;
    repository.element(Some("vendor/lib"), None)?;

    let verified = repository.verify("x")?;

    assert!(!marker.exists());
    assert_eq!(verified, Ok(head));
    Ok(())
}

/// An index of a nested repository that a check or Git refuses.
#[derive(Debug, Clone, Copy)]
enum BrokenIndex {
    /// Entries named `..` and `../x`.
    ParentPaths,
    /// A gitlink named `../../outside`.
    GitlinkOutside,
    /// Bytes that are no index.
    Corrupt,
}

/// Replaces the one occurrence of `from` in the file at `path` with `to`,
/// which has the same length.
fn patch(path: &Path, from: &[u8], to: &[u8]) -> TestResult {
    let bytes = fs::read(path)?;
    let found: Vec<usize> = bytes
        .windows(from.len())
        .enumerate()
        .filter(|(_, window)| *window == from)
        .map(|(at, _)| at)
        .collect();
    let [at] = found.as_slice() else {
        return Err(format!("{from:?} occurs {} times", found.len()).into());
    };
    let before = bytes.get(..*at).ok_or("no start")?;
    let after = bytes.get(at + from.len()..).ok_or("no end")?;
    fs::write(path, [before, to, after].concat())?;
    Ok(())
}

/// Breaks the index of the nested repository at `nested` as `case` says.
/// Git writes the index without a checksum, so the edited bytes are read;
/// the new names sort as the old ones did.
fn break_index(nested: &Path, case: BrokenIndex) -> TestResult {
    let index = nested.join(".git/index");
    git(nested, &["config", "index.skipHash", "true"])?;
    match case {
        BrokenIndex::ParentPaths => {
            fs::write(nested.join("-a"), "a\n")?;
            fs::create_dir(nested.join("-b"))?;
            fs::write(nested.join("-b/x"), "x\n")?;
            git(nested, &["add", "--", "-a", "-b/x"])?;
            patch(&index, b"-a\0", b"..\0")?;
            patch(&index, b"-b/x\0", b"../x\0")?;
        }
        BrokenIndex::GitlinkOutside => {
            let head = git(nested, &["rev-parse", "HEAD"])?;
            git(
                nested,
                &[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("160000,{head},-a/-b/outside"),
                ],
            )?;
            patch(&index, b"-a/-b/outside\0", b"../../outside\0")?;
        }
        BrokenIndex::Corrupt => {
            fs::write(&index, "not an index")?;
            return Ok(());
        }
    }
    let listed = git(nested, &["ls-files", "-s"])?;
    if !listed.contains("\t../") {
        return Err(format!("Git does not list the edited names: {listed}").into());
    }
    Ok(())
}

#[rstest]
#[case::paths_to_the_parent(BrokenIndex::ParentPaths)]
#[case::a_gitlink_outside(BrokenIndex::GitlinkOutside)]
#[case::a_corrupt_index(BrokenIndex::Corrupt)]
fn leaves_out_a_nested_repository_whose_index_it_refuses(#[case] case: BrokenIndex) -> TestResult {
    let sources = tempfile::tempdir()?;
    let stamp = library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(
        &sources.path().join("lib"),
        "vendor/lib",
        Nesting::Untracked,
    )?;
    break_index(&nested, case)?;
    repository.element(Some("vendor/lib/a.rs"), Some(&stamp))?;

    let flags = repository.flags()?;

    assert_eq!(flags, [Flag::UnknownStamp, Flag::AnchorMissing]);
    Ok(())
}

/// The refusal for an element `x` whose anchor lies in a nested repository
/// that Abyme cannot read.
fn unread() -> Result<String, String> {
    Err(
        "element x cannot be verified: its anchor lies in a nested repository \
         that Abyme cannot read"
            .to_owned(),
    )
}

#[rstest]
#[case::submodule(Nesting::Submodule)]
#[case::embedded_clone(Nesting::Embedded)]
#[case::untracked_clone(Nesting::Untracked)]
fn verify_stamps_the_head_of_the_nested_repository_and_the_review_reads_it_there(
    #[case] nesting: Nesting,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", nesting)?;
    fs::write(nested.join("c.rs"), "fn c() {}\n")?;
    let head = commit_all(&nested)?;
    repository.element(Some("vendor/lib"), None)?;

    let verified = repository.verify("x")?;

    assert_eq!(verified, Ok(head));
    assert_eq!(repository.flags()?, []);
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;
    assert_eq!(repository.flags()?, [changed(&[b"vendor/lib/a.rs"])]);
    Ok(())
}

/// A change under an anchor in a nested repository.
#[derive(Debug, Clone, Copy)]
enum NestedChange {
    InTheWorkingTree,
    Staged,
    Untracked,
}

#[rstest]
#[case::in_the_working_tree(NestedChange::InTheWorkingTree)]
#[case::staged(NestedChange::Staged)]
#[case::untracked(NestedChange::Untracked)]
fn verify_refuses_a_change_under_the_anchor_in_the_nested_repository(
    #[case] change: NestedChange,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(
        &sources.path().join("lib"),
        "vendor/lib",
        Nesting::Submodule,
    )?;
    repository.element(Some("vendor/lib"), None)?;
    match change {
        NestedChange::InTheWorkingTree => fs::write(nested.join("a.rs"), "fn a() { b() }\n")?,
        NestedChange::Staged => {
            fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;
            git(&nested, &["add", "a.rs"])?;
        }
        NestedChange::Untracked => fs::write(nested.join("new.rs"), "fn n() {}\n")?,
    }

    assert_eq!(repository.verify("x")?, uncommitted());
    Ok(())
}

#[rstest]
#[case::uninitialized_submodule(Elsewhere::UninitializedSubmodule)]
#[case::gitlink_without_git(Elsewhere::GitlinkWithoutGit)]
#[case::git_file_outside(Elsewhere::GitFileOutside)]
#[case::common_directory_outside(Elsewhere::CommonDirectoryOutside)]
#[case::git_symlink(Elsewhere::GitSymlink)]
#[case::work_tree_above(Elsewhere::WorkTreeAbove)]
#[case::work_tree_outside(Elsewhere::WorkTreeOutside)]
#[case::linked_work_tree(Elsewhere::LinkedWorkTree)]
#[case::symlinked_repository(Elsewhere::SymlinkedRepository)]
#[case::symlinked_parent(Elsewhere::SymlinkedParent)]
#[case::second_level_outside(Elsewhere::SecondLevelOutside)]
fn verify_refuses_an_anchor_in_a_nested_repository_that_abyme_cannot_read(
    #[case] case: Elsewhere,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    let lib = sources.path().join("lib");
    library(&lib)?;
    let outside_repository = sources.path().join("outside");
    outside(&outside_repository)?;
    let repository = Repository::new()?;
    let anchor = nest_elsewhere(&repository, &lib, &outside_repository, case)?;
    repository.element(Some(anchor), None)?;
    let before = repository.element_file("x")?;

    let verified = repository.verify("x")?;

    assert_eq!(verified, unread());
    assert_eq!(repository.element_file("x")?, before);
    Ok(())
}

#[rstest]
#[case::uninitialized_submodule(Elsewhere::UninitializedSubmodule)]
#[case::work_tree_outside(Elsewhere::WorkTreeOutside)]
fn verify_refuses_an_anchor_on_the_root_of_a_nested_repository_that_abyme_cannot_read(
    #[case] case: Elsewhere,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    let lib = sources.path().join("lib");
    library(&lib)?;
    let outside_repository = sources.path().join("outside");
    outside(&outside_repository)?;
    let repository = Repository::new()?;
    nest_elsewhere(&repository, &lib, &outside_repository, case)?;
    repository.element(Some("vendor/lib"), None)?;

    assert_eq!(repository.verify("x")?, unread());
    Ok(())
}

#[rstest]
#[case::clean_filter(Hostile::Clean)]
#[case::process_filter(Hostile::Process)]
#[case::odd_driver_name(Hostile::OddDriverName)]
#[case::included_filter(Hostile::Included)]
#[case::work_tree_configuration(Hostile::WorkTreeConfig)]
#[case::required_filter(Hostile::Required)]
#[case::info_attributes(Hostile::InfoAttributes)]
#[case::file_system_monitor(Hostile::Fsmonitor)]
#[case::index_hook(Hostile::Hook)]
fn verify_runs_nothing_that_a_nested_repository_s_own_configuration_sets_up(
    #[case] case: Hostile,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", Nesting::Embedded)?;
    let marker = sources.path().join("marker");
    make_hostile(&nested, case, &marker, sources.path())?;
    let head = git(&nested, &["rev-parse", "HEAD"])?;
    repository.element(Some("vendor/lib"), None)?;

    let verified = repository.verify("x")?;

    assert!(!marker.exists());
    assert_eq!(verified, Ok(head));
    Ok(())
}

#[rstest]
#[case::paths_to_the_parent(BrokenIndex::ParentPaths)]
#[case::a_gitlink_outside(BrokenIndex::GitlinkOutside)]
#[case::a_corrupt_index(BrokenIndex::Corrupt)]
fn verify_refuses_an_anchor_in_a_nested_repository_whose_index_it_refuses(
    #[case] case: BrokenIndex,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(
        &sources.path().join("lib"),
        "vendor/lib",
        Nesting::Untracked,
    )?;
    break_index(&nested, case)?;
    repository.element(Some("vendor/lib/a.rs"), None)?;
    let before = repository.element_file("x")?;

    let verified = repository.verify("x")?;

    assert_eq!(verified, unread());
    assert_eq!(repository.element_file("x")?, before);
    Ok(())
}

/// What happens in the nested repository under an anchor of the outer
/// repository that contains it.
#[derive(Debug, Clone, Copy)]
enum OuterCase {
    /// Files of a submodule change.
    DirtySubmodule,
    /// Files of an embedded clone change.
    DirtyEmbeddedClone,
    /// A submodule gets a new commit, so its gitlink differs from HEAD.
    MovedSubmodule,
    /// An embedded clone gets a new commit that the outer index stages.
    StagedEmbeddedClone,
}

#[rstest]
#[case::dirty_submodule(OuterCase::DirtySubmodule, false)]
#[case::dirty_embedded_clone(OuterCase::DirtyEmbeddedClone, false)]
#[case::moved_submodule(OuterCase::MovedSubmodule, true)]
#[case::staged_embedded_clone(OuterCase::StagedEmbeddedClone, true)]
fn verify_of_an_anchor_that_contains_a_nested_repository_refuses_only_a_moved_root(
    #[case] case: OuterCase,
    #[case] refused: bool,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nesting = match case {
        OuterCase::DirtySubmodule | OuterCase::MovedSubmodule => Nesting::Submodule,
        OuterCase::DirtyEmbeddedClone | OuterCase::StagedEmbeddedClone => Nesting::Embedded,
    };
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", nesting)?;
    repository.write(b"vendor/README.md", "Vendored code.\n")?;
    repository.element(Some("vendor"), None)?;
    let head = repository.commit()?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;
    match case {
        OuterCase::DirtySubmodule | OuterCase::DirtyEmbeddedClone => {}
        OuterCase::MovedSubmodule => {
            commit_all(&nested)?;
        }
        OuterCase::StagedEmbeddedClone => {
            commit_all(&nested)?;
            git(repository.top(), &["add", "vendor/lib"])?;
        }
    }

    let verified = repository.verify("x")?;

    assert_eq!(verified, if refused { uncommitted() } else { Ok(head) });
    Ok(())
}

#[test]
fn verify_stamps_a_repository_nested_two_levels_deep() -> TestResult {
    let sources = tempfile::tempdir()?;
    let (repository, stamp) = two_levels_deep(sources.path())?;
    repository.element(Some("vendor/lib/inner/a.rs"), None)?;

    assert_eq!(repository.verify("x")?, Ok(stamp));
    Ok(())
}

/// Adds a submodule at `path` of the repository at `outer`, cloned from
/// `source`, that the committed `.gitmodules` tells Git to ignore; then
/// gives the submodule a new commit and stages its moved gitlink. Returns
/// the commit that added the submodule.
fn ignored_submodule(outer: &Path, source: &Path, path: &str) -> TestResult<String> {
    submodule_add(outer, source, path)?;
    git(
        outer,
        &[
            "config",
            "-f",
            ".gitmodules",
            &format!("submodule.{path}.ignore"),
            "all",
        ],
    )?;
    let added = commit_all(outer)?;
    let submodule = outer.join(path);
    fs::write(submodule.join("a.rs"), "fn a() { b() }\n")?;
    commit_all(&submodule)?;
    git(outer, &["add", "--force", path])?;
    let staged = git(
        outer,
        &[
            "diff",
            "--cached",
            "--name-only",
            "--ignore-submodules=none",
        ],
    )?;
    if staged != path {
        return Err(format!("the moved gitlink is not staged: {staged:?}").into());
    }
    Ok(added)
}

#[test]
fn sees_a_moved_gitlink_that_gitmodules_says_to_ignore() -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let stamp = ignored_submodule(repository.top(), &sources.path().join("lib"), "vendor/lib")?;
    repository.element(Some("vendor"), Some(&stamp))?;

    let verified = repository.verify("x")?;
    git(repository.top(), &["commit", "-q", "-m", "move"])?;
    let flags = repository.flags()?;

    assert_eq!(verified, uncommitted());
    assert_eq!(flags, [changed(&[b"vendor/lib"])]);
    Ok(())
}

#[test]
fn sees_a_moved_gitlink_that_a_nested_repository_s_gitmodules_says_to_ignore() -> TestResult {
    let sources = tempfile::tempdir()?;
    let inner = sources.path().join("inner");
    library(&inner)?;
    let middle = sources.path().join("middle");
    library(&middle)?;
    let repository = Repository::new()?;
    let nested = repository.nest(&middle, "vendor/lib", Nesting::Untracked)?;
    let stamp = ignored_submodule(&nested, &inner, "inner")?;
    repository.element(Some("vendor/lib"), Some(&stamp))?;

    let verified = repository.verify("x")?;
    git(&nested, &["commit", "-q", "-m", "move"])?;
    let flags = repository.flags()?;

    assert_eq!(verified, uncommitted());
    assert_eq!(flags, [changed(&[b"vendor/lib/inner"])]);
    Ok(())
}

#[test]
fn verify_refuses_an_anchor_in_a_nested_repository_without_a_commit() -> TestResult {
    let repository = Repository::new()?;
    let nested = repository.project.join("vendor/lib");
    fs::create_dir_all(&nested)?;
    git(&nested, &["init", "-q"])?;
    fs::write(nested.join("a.rs"), "fn a() {}\n")?;
    repository.element(Some("vendor/lib/a.rs"), None)?;

    assert_eq!(
        repository.verify("x")?,
        Err("element x cannot be verified: the repository has no commit yet".to_owned())
    );
    Ok(())
}

/// How a history is rewritten after elements were stamped on the branch
/// `feature`.
#[derive(Debug, Clone, Copy)]
enum Rewrite {
    /// `feature` rebased onto `main`.
    Rebase,
    /// `feature` merged into `main` as one squashed commit.
    SquashMerge,
    /// The commits of `feature` picked onto `main`.
    CherryPick,
}

impl Repository {
    /// The elements `x`, anchored on `crates/x`, and `y`, anchored on
    /// `crates/y`, with `x` depending on `y`, stamped by `verify` on the
    /// branch `feature` and their stamps committed there; `main` gains a
    /// commit of its own that changes nothing under the anchors. Returns
    /// the commit the stamps name.
    fn stamped_on_a_branch(&self) -> TestResult<String> {
        git(self.top(), &["switch", "-q", "-c", "feature"])?;
        self.write(b"crates/x/lib.rs", "fn a() {}\n")?;
        self.write(b"crates/y/lib.rs", "fn b() {}\n")?;
        self.node("x", Some("crates/x"), None, &["y"])?;
        self.node("y", Some("crates/y"), None, &[])?;
        let stamped = self.commit()?;
        self.verify("x")??;
        self.verify("y")??;
        self.commit()?;
        git(self.top(), &["switch", "-q", "main"])?;
        self.write(b"other.rs", "fn o() {}\n")?;
        self.commit()?;
        Ok(stamped)
    }

    /// Rewrites the history of `feature` as `rewrite` says, ending on the
    /// branch that holds the result.
    fn rewrite(&self, rewrite: Rewrite) -> TestResult {
        match rewrite {
            Rewrite::Rebase => {
                git(self.top(), &["switch", "-q", "feature"])?;
                git(self.top(), &["rebase", "-q", "main"])?;
            }
            Rewrite::SquashMerge => {
                git(self.top(), &["merge", "-q", "--squash", "feature"])?;
                git(self.top(), &["commit", "-q", "-m", "squashed"])?;
            }
            Rewrite::CherryPick => {
                git(self.top(), &["cherry-pick", "main..feature"])?;
            }
        }
        Ok(())
    }

    /// Rewrites the commit HEAD names with another message, keeping its
    /// content and what is not committed.
    fn rewrite_head(&self) -> TestResult {
        git(self.top(), &["commit", "-q", "--amend", "-m", "rewritten"]).map(drop)
    }

    /// Whether `stamp` is no longer an ancestor of HEAD.
    fn is_rewritten(&self, stamp: &str) -> bool {
        git(self.top(), &["merge-base", "--is-ancestor", stamp, "HEAD"]).is_err()
    }

    /// Writes the element `y` on `crates/y`, whose recorded tree differs from
    /// HEAD's in `crates/y/a.rs` after two commits; returns the flags `y`
    /// gets for that once its stamp is judged by its content.
    fn sibling_with_a_file_list(&self) -> TestResult<Vec<Flag>> {
        self.write(b"crates/y/a.rs", "fn a() {}\n")?;
        self.commit()?;
        let recorded = git(self.top(), &["rev-parse", "HEAD:crates/y"])?;
        self.write(b"crates/y/a.rs", "fn a() { b() }\n")?;
        self.rewritten_element("y", "crates/y", Some(&recorded))?;
        self.commit()?;
        Ok(vec![changed(&[b"crates/y/a.rs"])])
    }

    /// The object the element file of `id` records for `path`.
    fn recorded_object(&self, id: &str, path: &str) -> TestResult<String> {
        let file = self.element_file(id)?;
        let start = format!("{{path: \"{path}\", object: \"");
        let at = file.find(&start).ok_or("no object recorded")? + start.len();
        Ok(file
            .get(at..)
            .ok_or("no object recorded")?
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect())
    }
}

#[rstest]
#[case::rebase(Rewrite::Rebase)]
#[case::squash_merge(Rewrite::SquashMerge)]
#[case::cherry_pick(Rewrite::CherryPick)]
fn flags_nothing_after_a_rewrite_that_changed_nothing_under_the_anchors(
    #[case] rewrite: Rewrite,
) -> TestResult {
    let repository = Repository::new()?;
    let stamped = repository.stamped_on_a_branch()?;

    repository.rewrite(rewrite)?;

    assert!(repository.is_rewritten(&stamped));
    assert_eq!(repository.flags_of("x")?, []);
    assert_eq!(repository.flags_of("y")?, []);
    Ok(())
}

/// A change after a rewrite.
#[derive(Debug, Clone, Copy)]
enum ChangeAfterRewrite {
    /// A staged change under the anchor of `x`.
    Staged,
    /// A change in the working tree under the anchor of `x`.
    WorkingTree,
    /// A committed change under the anchor of `x`.
    CommittedOwn,
    /// A committed change under the anchor of `y`, which `x` depends on.
    CommittedTarget,
}

#[rstest]
#[case::staged(ChangeAfterRewrite::Staged)]
#[case::working_tree(ChangeAfterRewrite::WorkingTree)]
#[case::committed_under_the_own_anchor(ChangeAfterRewrite::CommittedOwn)]
#[case::committed_under_the_target_s_anchor(ChangeAfterRewrite::CommittedTarget)]
fn flags_a_change_after_a_rewrite(#[case] change: ChangeAfterRewrite) -> TestResult {
    let repository = Repository::new()?;
    repository.stamped_on_a_branch()?;
    repository.rewrite(Rewrite::Rebase)?;
    let path: &[u8] = if matches!(change, ChangeAfterRewrite::CommittedTarget) {
        b"crates/y/lib.rs"
    } else {
        b"crates/x/lib.rs"
    };
    repository.write(path, "fn c() {}\n")?;
    match change {
        ChangeAfterRewrite::Staged => git(repository.top(), &["add", "-A"]).map(drop)?,
        ChangeAfterRewrite::WorkingTree => {}
        ChangeAfterRewrite::CommittedOwn | ChangeAfterRewrite::CommittedTarget => {
            repository.commit().map(drop)?;
        }
    }

    let flags = repository.flags_of("x")?;

    let expected = match change {
        ChangeAfterRewrite::Staged | ChangeAfterRewrite::WorkingTree => {
            vec![changed(&[b"crates/x/lib.rs"])]
        }
        ChangeAfterRewrite::CommittedOwn => vec![changed(&[b"crates/x/lib.rs"])],
        ChangeAfterRewrite::CommittedTarget => vec![Flag::Upstream {
            kind: RelationKind::DependsOn,
            target: "y".parse()?,
            priority: Priority::Task,
        }],
    };
    assert_eq!(flags, expected);
    Ok(())
}

#[test]
fn judges_a_stamp_on_another_branch_by_its_recorded_content() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.element(Some("crates/x"), None)?;
    repository.commit()?;
    git(repository.top(), &["switch", "-q", "-c", "side"])?;
    repository.write(b"side.rs", "fn s() {}\n")?;
    let side = repository.commit()?;
    repository.verify("x")??;
    git(repository.top(), &["switch", "-q", "main"])?;
    let unchanged = repository.flags()?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    repository.commit()?;

    let changed_since = repository.flags()?;

    assert!(repository.element_file("x")?.contains(&side));
    assert_eq!(unchanged, []);
    assert_eq!(changed_since, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}

#[test]
fn passes_a_flag_on_from_a_target_anchored_elsewhere_after_the_stamp() -> TestResult {
    let repository = Repository::new()?;
    let stamped = repository.stamped_on_a_branch()?;
    repository.rewrite(Rewrite::Rebase)?;
    repository.write(b"crates/z/lib.rs", "fn z() {}\n")?;
    repository.node("y", Some("crates/z"), Some(&stamped), &[])?;
    repository.commit()?;

    assert_eq!(
        repository.flags_of("x")?,
        [Flag::Upstream {
            kind: RelationKind::DependsOn,
            target: "y".parse()?,
            priority: Priority::Task,
        }]
    );
    Ok(())
}

#[test]
fn leaves_an_element_without_anchors_to_vouch_for_unflagged_after_a_rewrite() -> TestResult {
    let repository = Repository::new()?;
    let stamped = repository.stamped_on_a_branch()?;
    git(repository.top(), &["switch", "-q", "feature"])?;
    repository.node("w", None, None, &[])?;
    repository.node("old", None, Some(&stamped), &["y"])?;
    repository.node("old-anchored", Some("crates/x"), Some(&stamped), &["y"])?;
    repository.commit()?;
    repository.verify("w")??;
    repository.commit()?;
    git(repository.top(), &["switch", "-q", "main"])?;

    repository.rewrite(Rewrite::Rebase)?;

    assert!(
        repository
            .element_file("w")?
            .contains("verified_content: []\n")
    );
    assert_eq!(repository.flags_of("w")?, []);
    assert_eq!(repository.flags_of("old")?, [Flag::UnknownStamp]);
    assert_eq!(repository.flags_of("old-anchored")?, [Flag::UnknownStamp]);
    Ok(())
}

#[test]
fn compares_a_nested_root_by_its_tree_and_a_submodule_target_by_its_commit() -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(
        &sources.path().join("lib"),
        "vendor/lib",
        Nesting::Submodule,
    )?;
    repository.write(b"src/main.rs", "fn main() {}\n")?;
    repository.node("n", Some("vendor/lib"), None, &[])?;
    repository.node("s", Some("src"), None, &["t"])?;
    repository.node("t", Some("vendor/lib/a.rs"), None, &[])?;
    repository.commit()?;
    let nested_stamp = repository.verify("n")??;
    let outer_stamp = repository.verify("s")??;
    let kind = |directory: &Path, object: &str| git(directory, &["cat-file", "-t", object]);
    // The gitlink names a commit of the submodule, which only it holds.
    let tree = repository.recorded_object("n", "vendor/lib")?;
    let gitlink = repository.recorded_object("s", "vendor/lib")?;
    repository.rewrite_head()?;
    let after_the_outer_rewrite = repository.flags_of("s")?;
    git(&nested, &["commit", "-q", "--amend", "-m", "rewritten"])?;
    let after_the_nested_rewrite = repository.flags_of("n")?;
    let rewritten = (
        repository.is_rewritten(&outer_stamp),
        git(
            &nested,
            &["merge-base", "--is-ancestor", &nested_stamp, "HEAD"],
        )
        .is_err(),
    );
    repository.commit()?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;
    commit_all(&nested)?;
    repository.commit()?;

    let after_a_move = (repository.flags_of("n")?, repository.flags_of("s")?);

    assert_eq!(rewritten, (true, true));
    assert_eq!(kind(&nested, &tree)?, "tree");
    assert_eq!(kind(&nested, &gitlink)?, "commit");
    assert_eq!(after_the_outer_rewrite, []);
    assert_eq!(after_the_nested_rewrite, []);
    assert_eq!(
        after_a_move,
        (
            vec![changed(&[b"vendor/lib/a.rs"])],
            vec![Flag::Upstream {
                kind: RelationKind::DependsOn,
                target: "t".parse()?,
                priority: Priority::Task,
            }]
        )
    );
    Ok(())
}

#[test]
fn reads_a_rewritten_history_of_a_project_below_the_top_of_its_repository() -> TestResult {
    let repository = Repository::with_project(Path::new("a/b"))?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.element(Some("crates/x"), None)?;
    repository.commit()?;
    let stamped = repository.verify("x")??;
    repository.rewrite_head()?;
    repository.commit()?;
    let unchanged = repository.flags()?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    repository.commit()?;

    let changed_since = repository.flags()?;

    assert!(repository.is_rewritten(&stamped));
    assert_eq!(unchanged, []);
    assert_eq!(changed_since, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}

#[test]
fn never_hands_an_anchor_or_a_recorded_path_to_git_after_a_rewrite() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"--output=evil/a", "x\n")?;
    repository.write(b"--output=evil2/a", "y\n")?;
    repository.node("x", Some("--output=evil"), None, &["y"])?;
    repository.node("y", Some("--output=evil2"), None, &[])?;
    repository.commit()?;
    let stamp = repository.verify("x")??;
    repository.write(
        b"abyme/elements/z.md",
        &format!(
            "---\nid: \"z\"\ntype: \"t\"\nname: \"Z\"\nverified: \"{stamp}\"\nverified_by: \"ai\"\n\
             verified_content:\n  - {{path: \"--output=evil3\", object: \"{stamp}\"}}\n  \
             - {{path: \"crates/unasked\"}}\n---\n"
        ),
    )?;
    repository.rewrite_head()?;
    repository.commit()?;
    repository.write(b"--output=evil2/a", "z\n")?;
    repository.commit()?;

    let x = repository.flags_of("x")?;
    let z = repository.flags_of("z")?;

    assert!(repository.is_rewritten(&stamp));

    for evil in ["evil", "evil2", "evil3"] {
        assert!(!repository.project.join(evil).exists(), "{evil}");
    }
    assert_eq!(
        x,
        [Flag::Upstream {
            kind: RelationKind::DependsOn,
            target: "y".parse()?,
            priority: Priority::Task,
        }]
    );
    assert_eq!(z, []);
    Ok(())
}

#[rstest]
#[case::clean_filter(Hostile::Clean)]
#[case::process_filter(Hostile::Process)]
#[case::odd_driver_name(Hostile::OddDriverName)]
#[case::included_filter(Hostile::Included)]
#[case::work_tree_configuration(Hostile::WorkTreeConfig)]
#[case::required_filter(Hostile::Required)]
#[case::info_attributes(Hostile::InfoAttributes)]
#[case::file_system_monitor(Hostile::Fsmonitor)]
#[case::index_hook(Hostile::Hook)]
fn runs_nothing_that_a_nested_repository_sets_up_while_judging_recorded_content(
    #[case] case: Hostile,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", Nesting::Embedded)?;
    let marker = sources.path().join("marker");
    make_hostile(&nested, case, &marker, sources.path())?;
    let tree = git(&nested, &["rev-parse", "HEAD:"])?;
    repository.write(
        b"abyme/elements/x.md",
        &format!(
            "---\nid: \"x\"\ntype: \"t\"\nname: \"X\"\nanchor: \"vendor/lib\"\nverified: \"{UNKNOWN}\"\n\
             verified_by: \"ai\"\nverified_content:\n  - {{path: \"vendor/lib\", object: \"{tree}\"}}\n---\n"
        ),
    )?;

    let flags = repository.flags()?;

    assert!(!marker.exists());
    assert_eq!(flags, []);
    Ok(())
}

#[test]
fn fetches_nothing_for_a_partial_clone_while_judging_recorded_content() -> TestResult {
    let sources = tempfile::tempdir()?;
    let repository = Repository::new()?;
    let marker = sources.path().join("marker");
    partial_clone(&repository, sources.path(), &marker)?;
    repository.write(
        b"abyme/elements/x.md",
        &format!(
            "---\nid: \"x\"\ntype: \"t\"\nname: \"X\"\nanchor: \"vendor/lib/a.rs\"\nverified: \"{UNKNOWN}\"\n\
             verified_by: \"ai\"\nverified_content:\n  - {{path: \"vendor/lib/a.rs\", object: \"{UNKNOWN}\"}}\n---\n"
        ),
    )?;

    let flags = repository.flags()?;

    assert!(!marker.exists());
    assert_eq!(flags, [changed(&[b"vendor/lib/a.rs"])]);
    Ok(())
}

impl Repository {
    /// Writes the element `id` anchored on `anchor`, stamped with a commit
    /// no repository holds, so that its stamp is judged by its content,
    /// which records `object` for the anchor, or that nothing lay there.
    fn rewritten_element(&self, id: &str, anchor: &str, object: Option<&str>) -> TestResult {
        let entry = object.map_or_else(
            || format!("{{path: \"{anchor}\"}}"),
            |object| format!("{{path: \"{anchor}\", object: \"{object}\"}}"),
        );
        self.write(
            format!("abyme/elements/{id}.md").as_bytes(),
            &format!(
                "---\nid: \"{id}\"\ntype: \"t\"\nname: \"X\"\nanchor: \"{anchor}\"\n\
                 verified: \"{UNKNOWN}\"\nverified_by: \"ai\"\nverified_content:\n  - {entry}\n---\n"
            ),
        )
    }
}

#[test]
fn lists_the_files_under_a_folder_anchor_after_a_rewrite() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.write(b"crates/x/old.rs", "fn o() {}\n")?;
    repository.commit()?;
    let recorded = git(repository.top(), &["rev-parse", "HEAD:crates/x"])?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    fs::remove_file(repository.project.join("crates/x/old.rs"))?;
    repository.write(b"crates/x/sub/new.rs", "fn n() {}\n")?;
    repository.rewritten_element("x", "crates/x", Some(&recorded))?;
    repository.commit()?;
    repository.write(b"crates/x/staged.rs", "fn s() {}\n")?;
    git(repository.top(), &["add", "crates/x/staged.rs"])?;
    repository.write(b"crates/x/untracked.rs", "fn u() {}\n")?;

    let flags = repository.flags()?;

    assert_eq!(
        flags,
        [changed(&[
            b"crates/x/lib.rs",
            b"crates/x/old.rs",
            b"crates/x/staged.rs",
            b"crates/x/sub/new.rs",
            b"crates/x/untracked.rs",
        ])]
    );
    Ok(())
}

#[rstest]
#[case::the_file_s_former_object(None)]
#[case::a_missing_object(Some(UNKNOWN.to_owned()))]
#[case::an_object_of_another_length(Some("ab".repeat(32)))]
fn lists_a_file_anchor_after_a_rewrite(#[case] recorded: Option<String>) -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.commit()?;
    let recorded = match recorded {
        Some(recorded) => recorded,
        None => git(repository.top(), &["rev-parse", "HEAD:crates/x/lib.rs"])?,
    };
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    repository.rewritten_element("x", "crates/x/lib.rs", Some(&recorded))?;
    repository.commit()?;

    assert_eq!(repository.flags()?, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}

/// What a stamp records for a folder anchor that is no tree there.
#[derive(Debug, Clone, Copy)]
enum NoTree {
    Commit,
    Blob,
    AnnotatedTag,
    Missing,
    OtherLength,
    HeldNothing,
}

#[rstest]
#[case::a_commit(NoTree::Commit)]
#[case::a_blob(NoTree::Blob)]
#[case::an_annotated_tag(NoTree::AnnotatedTag)]
#[case::a_missing_object(NoTree::Missing)]
#[case::an_object_of_another_length(NoTree::OtherLength)]
#[case::nothing(NoTree::HeldNothing)]
fn tells_no_files_of_a_folder_anchor_whose_recorded_object_is_no_tree_there(
    #[case] kind: NoTree,
) -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.commit()?;
    let recorded = match kind {
        NoTree::Commit => Some(git(repository.top(), &["rev-parse", "HEAD"])?),
        NoTree::Blob => Some(git(
            repository.top(),
            &["rev-parse", "HEAD:crates/x/lib.rs"],
        )?),
        NoTree::AnnotatedTag => {
            git(repository.top(), &["tag", "-a", "-m", "tag", "v1"])?;
            Some(git(repository.top(), &["rev-parse", "v1"])?)
        }
        NoTree::Missing => Some(UNKNOWN.to_owned()),
        NoTree::OtherLength => Some("ab".repeat(32)),
        NoTree::HeldNothing => None,
    };
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    repository.rewritten_element("x", "crates/x", recorded.as_deref())?;
    let sibling = repository.sibling_with_a_file_list()?;

    assert_eq!(repository.flags()?, [Flag::UnknownStamp]);
    assert_eq!(repository.flags_of("y")?, sibling);
    Ok(())
}

#[test]
fn tells_no_files_from_a_tree_only_another_repository_holds() -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.commit()?;
    let outer_tree = git(repository.top(), &["rev-parse", "HEAD:crates/x"])?;
    let sibling = repository.sibling_with_a_file_list()?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", Nesting::Embedded)?;
    fs::write(nested.join("a.rs"), "fn a() { b() }\n")?;
    commit_all(&nested)?;
    repository.rewritten_element("x", "vendor/lib", Some(&outer_tree))?;

    assert_eq!(repository.flags()?, [Flag::UnknownStamp]);
    assert_eq!(repository.flags_of("y")?, sibling);
    Ok(())
}

/// A source repository at `sources/deep` whose first commit holds
/// `src/a.rs` and whose second changes it, cloned without trees to
/// `vendor/lib` of `repository` with an SSH remote whose command leaves
/// `marker` behind; returns the root tree of the first commit and the
/// clone's directory.
fn deep_partial_clone(
    repository: &Repository,
    sources: &Path,
    marker: &Path,
) -> TestResult<(String, PathBuf)> {
    let source = sources.join("deep");
    fs::create_dir_all(source.join("src"))?;
    git(&source, &["init", "-q"])?;
    fs::write(source.join("src/a.rs"), "fn a() {}\n")?;
    commit_all(&source)?;
    let first_tree = git(&source, &["rev-parse", "HEAD^{tree}"])?;
    fs::write(source.join("src/a.rs"), "fn a() { b() }\n")?;
    commit_all(&source)?;
    git(&source, &["config", "uploadpack.allowFilter", "true"])?;
    git(
        repository.top(),
        &[
            "clone",
            "-q",
            "--filter=tree:0",
            &format!("file://{}", source.display()),
            "vendor/lib",
        ],
    )?;
    let nested = repository.project.join("vendor/lib");
    git(
        &nested,
        &["remote", "set-url", "origin", "ssh://example.invalid/lib"],
    )?;
    git(
        &nested,
        &[
            "config",
            "core.sshCommand",
            &format!("sh -c 'touch {}'", marker.display()),
        ],
    )?;
    git(&nested, &["config", "protocol.ssh.allow", "always"])?;
    Ok((first_tree, nested))
}

#[test]
fn fetches_nothing_for_a_recorded_tree_a_partial_clone_lacks() -> TestResult {
    let sources = tempfile::tempdir()?;
    let repository = Repository::new()?;
    let marker = sources.path().join("marker");
    let sibling = repository.sibling_with_a_file_list()?;
    let (first_tree, _) = deep_partial_clone(&repository, sources.path(), &marker)?;
    repository.rewritten_element("x", "vendor/lib", Some(&first_tree))?;

    let flags = repository.flags()?;

    assert!(!marker.exists());
    assert_eq!(flags, [Flag::UnknownStamp]);
    assert_eq!(repository.flags_of("y")?, sibling);
    Ok(())
}

#[test]
fn fetches_nothing_for_the_subtrees_of_a_recorded_tree_a_partial_clone_holds() -> TestResult {
    let sources = tempfile::tempdir()?;
    let repository = Repository::new()?;
    let marker = sources.path().join("marker");
    let sibling = repository.sibling_with_a_file_list()?;
    let (first_tree, nested) = deep_partial_clone(&repository, sources.path(), &marker)?;
    let content = Command::new("git")
        .current_dir(sources.path().join("deep"))
        .args(["cat-file", "tree", &first_tree])
        .output()?
        .stdout;
    let mut write = Command::new("git")
        .current_dir(&nested)
        .args(["hash-object", "-t", "tree", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    write.stdin.take().ok_or("no stdin")?.write_all(&content)?;
    let written = String::from_utf8(write.wait_with_output()?.stdout)?;
    repository.rewritten_element("x", "vendor/lib", Some(&first_tree))?;

    let flags = repository.flags()?;

    assert_eq!(written.trim_end(), first_tree);
    assert!(!marker.exists());
    assert_eq!(flags, [Flag::UnknownStamp]);
    assert_eq!(repository.flags_of("y")?, sibling);
    Ok(())
}

#[rstest]
#[case::clean_filter(Hostile::Clean)]
#[case::process_filter(Hostile::Process)]
#[case::odd_driver_name(Hostile::OddDriverName)]
#[case::included_filter(Hostile::Included)]
#[case::work_tree_configuration(Hostile::WorkTreeConfig)]
#[case::required_filter(Hostile::Required)]
#[case::info_attributes(Hostile::InfoAttributes)]
#[case::file_system_monitor(Hostile::Fsmonitor)]
#[case::index_hook(Hostile::Hook)]
fn runs_nothing_that_a_nested_repository_sets_up_while_listing_its_files(
    #[case] case: Hostile,
) -> TestResult {
    let sources = tempfile::tempdir()?;
    library(&sources.path().join("lib"))?;
    let repository = Repository::new()?;
    let nested = repository.nest(&sources.path().join("lib"), "vendor/lib", Nesting::Embedded)?;
    let recorded = git(&nested, &["rev-parse", "HEAD:"])?;
    fs::write(nested.join("c.rs"), "fn c() {}\n")?;
    commit_all(&nested)?;
    let marker = sources.path().join("marker");
    make_hostile(&nested, case, &marker, sources.path())?;
    repository.rewritten_element("x", "vendor/lib", Some(&recorded))?;

    let flags = repository.flags()?;

    assert!(!marker.exists());
    let [Flag::Changed(files)] = flags.as_slice() else {
        return Err(format!("{flags:?}").into());
    };
    assert!(files.contains(&b"vendor/lib/c.rs".to_vec()), "{files:?}");
    Ok(())
}

#[test]
fn lists_for_each_recorded_tree_of_one_anchor_its_own_files() -> TestResult {
    let repository = Repository::new()?;
    repository.write(b"crates/x/a.rs", "fn a() {}\n")?;
    repository.commit()?;
    let first = git(repository.top(), &["rev-parse", "HEAD:crates/x"])?;
    repository.write(b"crates/x/b.rs", "fn b() {}\n")?;
    repository.commit()?;
    let second = git(repository.top(), &["rev-parse", "HEAD:crates/x"])?;
    repository.write(b"crates/x/a.rs", "fn a() { b() }\n")?;
    repository.rewritten_element("x", "crates/x", Some(&first))?;
    repository.rewritten_element("w", "crates/x", Some(&second))?;
    repository.rewritten_element("v", "crates/x", Some(UNKNOWN))?;
    repository.commit()?;

    assert_eq!(
        repository.flags_of("x")?,
        [changed(&[b"crates/x/a.rs", b"crates/x/b.rs"])]
    );
    assert_eq!(repository.flags_of("w")?, [changed(&[b"crates/x/a.rs"])]);
    assert_eq!(repository.flags_of("v")?, [Flag::UnknownStamp]);
    Ok(())
}

#[test]
fn lists_a_file_anchor_stamped_by_verify_after_a_rebase() -> TestResult {
    let repository = Repository::new()?;
    git(repository.top(), &["switch", "-q", "-c", "feature"])?;
    repository.write(b"crates/x/lib.rs", "fn a() {}\n")?;
    repository.element(Some("crates/x/lib.rs"), None)?;
    let stamped = repository.commit()?;
    repository.verify("x")??;
    repository.commit()?;
    git(repository.top(), &["switch", "-q", "main"])?;
    repository.write(b"other.rs", "fn o() {}\n")?;
    repository.commit()?;
    repository.rewrite(Rewrite::Rebase)?;
    repository.write(b"crates/x/lib.rs", "fn a() { b() }\n")?;
    repository.commit()?;

    let flags = repository.flags()?;

    assert!(repository.is_rewritten(&stamped));
    assert_eq!(flags, [changed(&[b"crates/x/lib.rs"])]);
    Ok(())
}
