//! What Git reports about the project's files, read through the `git`
//! command line.
//!
//! Element files are untrusted, so no anchor or recorded path ever reaches
//! Git's command line: they are matched against the paths and objects Git
//! prints. Only hex-checked object IDs do, after all options: a stamp's
//! commit, the commit HEAD names, and, after a rewrite, an object a stamp
//! recorded for its anchor, which first goes to `cat-file --batch-check` on
//! standard input and reaches the command line only once Git has echoed
//! exactly that ID as a tree. Git runs
//! without a shell, with every `GIT_` variable of the environment removed,
//! without transports, and only with commands that leave the repository and
//! its index as they are. It never uses a bare repository it finds on its
//! own: a clone can bring the files of one, whose configuration would run
//! commands. What Git prints on errors never reaches a reader.
//!
//! Repositories nested in the project are found only through what Git
//! prints, and read only when an anchor lies in them. Git runs in one only
//! when every directory down to it is a real one, Git names it as the top
//! of its work tree, and its Git directory is its own or lies in the
//! outer repository's store of submodules; its own filters and hooks are
//! turned off there, and every path it prints is checked. A nested
//! repository in which a check or Git fails is left out with everything
//! nested in it, so its anchors match no file, and `verify` refuses an
//! anchor in it.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write as _;
use std::iter;
use std::ops::Bound;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;

use abyme_model::{Anchor, CommitId, ObjectId, RecordedContent};

use crate::review::{
    Fallback, GitFacts, HeadFacts, RepositoryFacts, Since, Unavailable, repository_root,
};

/// The environment every Git command runs with, after every `GIT_`
/// variable is removed: no index writes, no prompts, no fetches, no
/// transports, and no replaced objects.
const ENVIRONMENT: [(&str, &str); 5] = [
    ("GIT_OPTIONAL_LOCKS", "0"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_NO_LAZY_FETCH", "1"),
    ("GIT_NO_REPLACE_OBJECTS", "1"),
    ("GIT_ALLOW_PROTOCOL", ""),
];

/// The settings of a filter driver that a nested repository's own
/// configuration sets up, overridden so that the driver runs nothing.
const FILTER_OVERRIDES: [(&str, &str); 3] = [("clean", ""), ("process", ""), ("required", "false")];

/// What Git is asked about for one element: where its anchor lies, which
/// commit its stamp names, and the content the stamp recorded, if any.
pub struct AnchorAndStamp<'a> {
    pub anchor: Option<&'a Anchor>,
    pub stamp: Option<&'a CommitId>,
    pub content: Option<&'a RecordedContent>,
}

/// An entry of a tree: the type of what it holds and its object.
type TreeEntry = (ObjectKind, ObjectId);

/// What a path holds in a commit, as `ls-tree` names its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectKind {
    /// A folder.
    Tree,
    /// A file or a symbolic link.
    Blob,
    /// The commit a nested repository is at.
    Gitlink,
}

/// An element whose stamp's commit tells nothing and whose anchor holds
/// another object at HEAD than the stamp recorded, with how the files that
/// differ can be told.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Rewritten<'a> {
    /// HEAD holds a file at the anchor: the anchor itself differs.
    File {
        anchor: &'a [u8],
        recorded: &'a ObjectId,
    },
    /// HEAD holds a folder at the anchor, and the recorded object has the
    /// length of the repository's object IDs: the files that differ between
    /// the recorded tree and HEAD's, once Git confirms the recorded object
    /// is a tree.
    Trees {
        anchor: &'a [u8],
        recorded: &'a ObjectId,
        head: &'a ObjectId,
    },
}

/// The `git` program, found in a directory of `PATH` given as an absolute
/// path, so a relative entry cannot pick a `git` from the project.
struct Git {
    program: PathBuf,
}

/// Where a Git command runs: the project root, or the top of a nested
/// repository with the settings that hold there.
struct Place {
    /// The directory Git runs in.
    directory: PathBuf,
    /// The root of the nested repository, relative to the project root;
    /// `None` for the project's repository.
    root: Option<Vec<u8>>,
    /// The variables set beyond [`ENVIRONMENT`].
    settings: Vec<(OsString, OsString)>,
}

/// What one repository's listing tells. Paths are relative to the project
/// root.
#[derive(Debug, PartialEq, Eq)]
struct Listing {
    /// The paths that Git tracks or lists as untracked and not ignored,
    /// the roots of nested repositories included.
    known: BTreeSet<Vec<u8>>,
    /// The paths whose working tree differs from the index, that are
    /// unmerged, or that are untracked; never the root of an untracked
    /// repository.
    uncommitted: BTreeSet<Vec<u8>>,
    /// The roots of the nested repositories: gitlinks and untracked
    /// repositories.
    repositories: BTreeSet<Vec<u8>>,
}

/// Where a repository keeps its Git directory, its common directory, and
/// the Git directories of its submodules, as Git names them.
struct Layout {
    git: PathBuf,
    common: PathBuf,
    store: PathBuf,
}

/// What a walk through the nested repositories needs at every level.
struct Walk<'a> {
    git: &'a Git,
    /// The project root.
    root: &'a Path,
}

/// The paths review looks up in a repository's content after a rewrite:
/// the anchors of all elements and every folder above one. They must cover
/// every path review looks up through `recorded_matches`, an own anchor or
/// the path that stands for a relation target, its anchor or the root of a
/// nested repository above it: a stamp that recorded a path as holding
/// nothing would match a path left out, and a change would go unflagged.
/// Only the anchors are held, borrowed, so the set grows with their bytes
/// and not with the folders above them.
struct Lookups<'a> {
    anchors: BTreeSet<&'a [u8]>,
}

impl<'a> Lookups<'a> {
    /// The paths review looks up for the `elements`.
    fn new(elements: &[AnchorAndStamp<'a>]) -> Self {
        Self {
            anchors: elements
                .iter()
                .filter_map(|element| element.anchor)
                .map(|anchor| anchor.as_str().as_bytes())
                .collect(),
        }
    }

    /// Whether `path`, relative to the project root, is an anchor or a
    /// folder above one. The anchors below a folder sort together right
    /// from the folder's path with a `/`, so the first anchor from there
    /// tells.
    fn keep(&self, path: &[u8]) -> bool {
        if self.anchors.contains(path) {
            return true;
        }
        let folder = [path, b"/"].concat();
        self.anchors
            .range::<[u8], _>((Bound::Included(folder.as_slice()), Bound::Unbounded))
            .next()
            .is_some_and(|anchor| anchor.starts_with(&folder))
    }
}

/// Which entries of a tree listing are kept.
#[derive(Clone, Copy)]
enum Kept<'a> {
    /// Every entry, as `verify` needs them to record what a stamp vouches
    /// for.
    All,
    /// The entries review looks up after a rewrite.
    Lookups(&'a Lookups<'a>),
}

impl Kept<'_> {
    /// Whether the entry at `path`, relative to the project root, is kept.
    fn keeps(self, path: &[u8]) -> bool {
        match self {
            Self::All => true,
            Self::Lookups(lookups) => lookups.keep(path),
        }
    }
}

/// What Git reports about the files of the project at `root`, of the
/// repositories nested in it in which an element's anchor lies, and about
/// each element's stamp, in the element's repository.
pub fn facts(root: &Path, elements: &[AnchorAndStamp<'_>]) -> Result<GitFacts, Unavailable> {
    let git = Git::find()?;
    let project = Place::project(root);
    let prefix = git.prefix(&project)?;
    let listing = git.listing(&project, &prefix)?;
    let wanted = wanted(&listing.repositories, elements);
    let lookups = Lookups::new(elements);
    let nested = if wanted.is_empty() {
        BTreeMap::new()
    } else {
        let store = git.store(&project)?;
        Walk { git: &git, root }.nested(&wanted, store.as_deref(), elements, &lookups)
    };
    let members = members(None, &nested, elements);
    Ok(GitFacts {
        project: git.repository_facts(&project, listing, &members, &lookups)?,
        nested,
    })
}

/// What `verify` needs to know of the repository an element with `anchor`
/// belongs to in the project at `root`: the deepest repository on the
/// anchor's chain of nested repositories, or the project's. That is the
/// commit its HEAD names, and the paths that differ from that commit in
/// the index or the working tree, or are untracked; the root of an
/// untracked repository is none of them. `None` when the anchor lies in a
/// nested repository that Git may not run in, or in which Git fails: the
/// project's repository never stands in for it.
pub fn head(root: &Path, anchor: Option<&Anchor>) -> Result<Option<HeadFacts>, Unavailable> {
    let git = Git::find()?;
    let project = Place::project(root);
    let prefix = git.prefix(&project)?;
    let listing = git.listing(&project, &prefix)?;
    let Some(anchor) =
        anchor.filter(|anchor| repository_root(&listing.repositories, Some(anchor)).is_some())
    else {
        let roots = listing.repositories.clone();
        return git
            .head_facts(&project, listing.uncommitted, roots)
            .map(Some);
    };
    let store = git.store(&project)?;
    let roots = listing.repositories.clone();
    Ok(Walk { git: &git, root }
        .deepest(project, listing, store.as_deref(), anchor, roots)
        .and_then(|(place, listing, roots)| {
            git.head_facts(&place, listing.uncommitted, roots).ok()
        }))
}

impl Walk<'_> {
    /// The facts of the nested repositories at `roots` and of those nested
    /// in them in which an anchor of the `elements` lies, by root. `store` is
    /// where their outer repository keeps the Git directories of its
    /// submodules. A repository that Git may not run in, or in which Git
    /// fails, is left out with everything nested in it. `lookups` are the
    /// paths review looks up after a rewrite, for every element.
    fn nested(
        &self,
        roots: &[&Vec<u8>],
        store: Option<&Path>,
        elements: &[AnchorAndStamp<'_>],
        lookups: &Lookups<'_>,
    ) -> BTreeMap<Vec<u8>, RepositoryFacts> {
        roots
            .iter()
            .filter_map(|root| self.repository(root, store, elements, lookups))
            .flatten()
            .collect()
    }

    /// The facts of the nested repository at `root` and of those nested in
    /// it, or `None` when Git may not run there or fails.
    fn repository(
        &self,
        root: &[u8],
        store: Option<&Path>,
        elements: &[AnchorAndStamp<'_>],
        lookups: &Lookups<'_>,
    ) -> Option<BTreeMap<Vec<u8>, RepositoryFacts>> {
        let (place, own_store) = self.enter(root, store)?;
        let listing = self.git.listing(&place, b"").ok()?;
        let wanted = wanted(&listing.repositories, elements);
        let mut nested = self.nested(&wanted, own_store.as_deref(), elements, lookups);
        let members = members(Some(root), &nested, elements);
        let facts = self
            .git
            .repository_facts(&place, listing, &members, lookups)
            .ok()?;
        nested.insert(root.to_vec(), facts);
        Some(nested)
    }

    /// The deepest repository on the chain of nested repositories that
    /// `anchor` lies in, starting at the one at `place` with its `listing`
    /// and its `store` of submodules, with that repository's listing and the
    /// `roots` of nested repositories seen so far together with those its
    /// listing shows; `None` when a repository on the chain is one Git may
    /// not run in, or in which Git fails.
    fn deepest(
        &self,
        place: Place,
        listing: Listing,
        store: Option<&Path>,
        anchor: &Anchor,
        roots: BTreeSet<Vec<u8>>,
    ) -> Option<(Place, Listing, BTreeSet<Vec<u8>>)> {
        let Some(root) = repository_root(&listing.repositories, Some(anchor)) else {
            return Some((place, listing, roots));
        };
        let (nested, own_store) = self.enter(root, store)?;
        let nested_listing = self.git.listing(&nested, b"").ok()?;
        let roots = roots
            .into_iter()
            .chain(nested_listing.repositories.iter().cloned())
            .collect();
        self.deepest(nested, nested_listing, own_store.as_deref(), anchor, roots)
    }

    /// The place to run Git in the nested repository at `root`, and where
    /// it keeps the Git directories of its submodules; `None` unless every
    /// directory down to it is a real one, Git names it as the top of its
    /// work tree, its Git directory is its own `.git` directory or lies in
    /// `store`, and its common directory is its Git directory. Its own
    /// filters and hooks are turned off there.
    fn enter(&self, root: &[u8], store: Option<&Path>) -> Option<(Place, Option<PathBuf>)> {
        let directory = real_directory(self.root, root)?;
        let top = fs::canonicalize(&directory).ok()?;
        let ceiling = top.parent()?.to_path_buf();
        let place = Place {
            directory,
            root: Some(root.to_vec()),
            settings: nested_settings(&ceiling, &BTreeSet::new()),
        };
        let layout = self.git.layout(&place).ok()?;
        let git_directory = fs::canonicalize(&layout.git).ok()?;
        let own =
            git_directory == top.join(".git") && is_real_directory(&place.directory.join(".git"));
        let stored =
            store.is_some_and(|store| git_directory.starts_with(store) && git_directory != store);
        if !(own || stored) || fs::canonicalize(&layout.common).ok()? != git_directory {
            return None;
        }
        let drivers = self.git.untrusted_filters(&place).ok()?;
        Some((
            Place {
                settings: nested_settings(&ceiling, &drivers),
                ..place
            },
            fs::canonicalize(&layout.store).ok(),
        ))
    }
}

impl Place {
    fn project(root: &Path) -> Self {
        Self {
            directory: root.to_path_buf(),
            root: None,
            settings: Vec::new(),
        }
    }

    /// `path`, which Git printed relative to this place, relative to the
    /// project root: as it is in the project's repository, checked and put
    /// below the root in a nested one.
    fn path(&self, path: &[u8]) -> Result<Vec<u8>, Unavailable> {
        self.root
            .as_deref()
            .map_or_else(|| Ok(path.to_vec()), |root| nested_path(root, path))
    }

    /// `paths`, which Git printed relative to this place, relative to the
    /// project root.
    fn paths<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a [u8]>,
    ) -> Result<BTreeSet<Vec<u8>>, Unavailable> {
        paths.into_iter().map(|path| self.path(path)).collect()
    }
}

impl Git {
    fn find() -> Result<Self, Unavailable> {
        env::var_os("PATH")
            .iter()
            .flat_map(env::split_paths)
            .filter(|directory| directory.is_absolute())
            .map(|directory| directory.join("git"))
            .find(|program| program.is_file())
            .map(|program| Self { program })
            .ok_or(Unavailable::NoGit)
    }

    /// Runs `git` at `place` with `arguments`.
    fn run(&self, place: &Place, arguments: &[&str]) -> Result<Output, Unavailable> {
        self.command(place, arguments)
            .output()
            .map_err(|_| Unavailable::NoGit)
    }

    /// Runs `git` at `place` with `arguments`, writing `input` to its
    /// standard input from a thread of its own while its output is read, so
    /// that neither side waits on a full pipe. A failure to write fails the
    /// run.
    fn run_with_input(
        &self,
        place: &Place,
        arguments: &[&str],
        input: &[u8],
    ) -> Result<Output, Unavailable> {
        let mut child = self
            .command(place, arguments)
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|_| Unavailable::NoGit)?;
        let mut stdin = child.stdin.take().ok_or(Unavailable::GitFailed)?;
        thread::scope(|scope| {
            let writer = scope.spawn(move || stdin.write_all(input));
            let output = child.wait_with_output();
            match (output, writer.join()) {
                (Ok(output), Ok(Ok(()))) => Ok(output),
                _ => Err(Unavailable::GitFailed),
            }
        })
    }

    /// The `git` command at `place` with `arguments`, in its environment.
    fn command(&self, place: &Place, arguments: &[&str]) -> Command {
        let mut command = Command::new(&self.program);
        command
            .current_dir(&place.directory)
            .args([
                "-c",
                "core.fsmonitor=false",
                "-c",
                "safe.bareRepository=explicit",
            ])
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, _) in env::vars_os() {
            if name.as_encoded_bytes().starts_with(b"GIT_") {
                command.env_remove(name);
            }
        }
        command
            .envs(ENVIRONMENT)
            .envs(place.settings.iter().cloned());
        command
    }

    /// What `git` prints when it succeeds.
    fn success(&self, place: &Place, arguments: &[&str]) -> Result<Vec<u8>, Unavailable> {
        let output = self.run(place, arguments)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(Unavailable::GitFailed)
        }
    }

    /// Where the project root lies in its work tree, as Git prints paths:
    /// empty at the top, otherwise ending with `/`.
    fn prefix(&self, place: &Place) -> Result<Vec<u8>, Unavailable> {
        let output = self.run(
            place,
            &["rev-parse", "--is-inside-work-tree", "--show-prefix"],
        )?;
        if !output.status.success() {
            return Err(Unavailable::NoRepository);
        }
        output
            .stdout
            .strip_prefix(b"true\n")
            .ok_or(Unavailable::NoRepository)?
            .strip_suffix(b"\n")
            .map(<[u8]>::to_vec)
            .ok_or(Unavailable::GitFailed)
    }

    /// Where the repository at `place` keeps the Git directories of its
    /// submodules, if that directory exists.
    fn store(&self, place: &Place) -> Result<Option<PathBuf>, Unavailable> {
        let output = self.success(
            place,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                "modules",
            ],
        )?;
        let store = output.strip_suffix(b"\n").ok_or(Unavailable::GitFailed)?;
        Ok(fs::canonicalize(OsStr::from_bytes(store)).ok())
    }

    /// The layout of the repository at `place`, if Git names `place` as the
    /// top of its work tree.
    fn layout(&self, place: &Place) -> Result<Layout, Unavailable> {
        let output = self.success(
            place,
            &[
                "rev-parse",
                "--is-inside-work-tree",
                "--show-prefix",
                "--path-format=absolute",
                "--absolute-git-dir",
                "--git-common-dir",
                "--git-path",
                "modules",
            ],
        )?;
        let lines: Vec<&[u8]> = output
            .strip_prefix(b"true\n\n")
            .and_then(|lines| lines.strip_suffix(b"\n"))
            .ok_or(Unavailable::GitFailed)?
            .split(|byte| *byte == b'\n')
            .collect();
        match lines.as_slice() {
            [git, common, store] => Ok(Layout {
                git: PathBuf::from(OsStr::from_bytes(git)),
                common: PathBuf::from(OsStr::from_bytes(common)),
                store: PathBuf::from(OsStr::from_bytes(store)),
            }),
            _ => Err(Unavailable::GitFailed),
        }
    }

    /// The filter drivers at `place` for which a configuration other than
    /// the user's global one or the system's sets a key.
    fn untrusted_filters(&self, place: &Place) -> Result<BTreeSet<Vec<u8>>, Unavailable> {
        let output = self.run(
            place,
            &[
                "config",
                "--show-scope",
                "--name-only",
                "-z",
                "--get-regexp",
                r"^filter\.",
            ],
        )?;
        match output.status.code() {
            Some(0) => untrusted_drivers(&output.stdout),
            Some(1) if output.stdout.is_empty() => Ok(BTreeSet::new()),
            _ => Err(Unavailable::GitFailed),
        }
    }

    /// The listing of the repository at `place`, whose paths `git status`
    /// prints with `prefix` in front.
    fn listing(&self, place: &Place, prefix: &[u8]) -> Result<Listing, Unavailable> {
        let output = self.success(place, &["ls-files", "-z", "-s"])?;
        let staged = staged(&output)?;
        let tracked = place.paths(staged.iter().map(|(_, path)| *path))?;
        let gitlinks = place.paths(
            staged
                .iter()
                .filter(|(gitlink, _)| *gitlink)
                .map(|(_, path)| *path),
        )?;
        let Status {
            uncommitted,
            untracked,
            repositories,
        } = self.status(place, prefix)?;
        Ok(Listing {
            known: tracked
                .into_iter()
                .chain(untracked)
                .chain(repositories.iter().cloned())
                .collect(),
            uncommitted,
            repositories: gitlinks.into_iter().chain(repositories).collect(),
        })
    }

    /// What `verify` needs to know of the repository at `place`, whose
    /// working tree differs from its index in `uncommitted`, with the `roots`
    /// of nested repositories seen on the way to it: the commit HEAD names,
    /// the paths that differ from that commit, and the object each path held
    /// in it. Every query names the commit, resolved once, never `HEAD`, so
    /// that the objects belong to the commit the stamp names.
    fn head_facts(
        &self,
        place: &Place,
        uncommitted: BTreeSet<Vec<u8>>,
        roots: BTreeSet<Vec<u8>>,
    ) -> Result<HeadFacts, Unavailable> {
        let Some(commit) = self.commit(place, "HEAD")? else {
            return Ok(HeadFacts {
                commit: None,
                changed: uncommitted,
                root: place.root.clone(),
                roots,
                objects: BTreeMap::new(),
            });
        };
        let staged = self.changed_in_index(place, &commit)?;
        let objects = self.objects(place, &commit)?;
        Ok(HeadFacts {
            commit: Some(commit),
            changed: staged.into_iter().chain(uncommitted).collect(),
            root: place.root.clone(),
            roots,
            objects,
        })
    }

    /// The object each path held in `commit` of the repository at `place`:
    /// every file, folder, and nested repository below the project root,
    /// and the repository's own root tree when it is nested. Git lists the
    /// tree with the commit named after all options and no path, so nothing
    /// from an element file reaches its command line.
    fn objects(
        &self,
        place: &Place,
        commit: &CommitId,
    ) -> Result<BTreeMap<Vec<u8>, ObjectId>, Unavailable> {
        Ok(self
            .typed_objects(place, commit, Kept::All)?
            .into_iter()
            .map(|(path, (_, object))| (path, object))
            .collect())
    }

    /// The object each path that `kept` keeps held in `commit` of the
    /// repository at `place`, as [`Git::objects`] reads them, each with its
    /// type. Every entry Git prints is checked, kept or not.
    fn typed_objects(
        &self,
        place: &Place,
        commit: &CommitId,
        kept: Kept<'_>,
    ) -> Result<BTreeMap<Vec<u8>, TreeEntry>, Unavailable> {
        let output = self.success(place, &["ls-tree", "-r", "-t", "-z", commit.as_str()])?;
        let mut objects = tree_objects(&output, place.root.as_deref(), kept)?;
        if let Some(root) = place.root.as_ref().filter(|root| kept.keeps(root)) {
            objects.insert(
                root.clone(),
                (ObjectKind::Tree, self.root_tree(place, commit)?),
            );
        }
        Ok(objects)
    }

    /// The tree of `commit` in the repository at `place`; anything but one
    /// object ID fails.
    fn root_tree(&self, place: &Place, commit: &CommitId) -> Result<ObjectId, Unavailable> {
        let output = self.success(
            place,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{commit}^{{tree}}"),
            ],
        )?;
        output
            .strip_suffix(b"\n")
            .and_then(|id| String::from_utf8(id.to_vec()).ok())
            .and_then(|id| ObjectId::try_from(id).ok())
            .ok_or(Unavailable::GitFailed)
    }

    /// The facts of the repository at `place` from its `listing`, with what
    /// the stamp of each of its `members` tells there, and its content at
    /// HEAD when a stamp that recorded content names a commit that tells
    /// nothing.
    fn repository_facts(
        &self,
        place: &Place,
        listing: Listing,
        members: &[&AnchorAndStamp<'_>],
        lookups: &Lookups<'_>,
    ) -> Result<RepositoryFacts, Unavailable> {
        let head = self.commit(place, "HEAD")?;
        let stamps = stamps(members);
        let since = stamps
            .keys()
            .map(|stamp| Ok(((*stamp).clone(), self.since(place, stamp, head.as_ref())?)))
            .collect::<Result<BTreeMap<_, _>, Unavailable>>()?;
        let needed = stamps
            .iter()
            .any(|(stamp, recorded)| *recorded && since.get(*stamp) == Some(&Since::Unknown));
        let fallback = match head.filter(|_| needed) {
            Some(head) => {
                let objects = self.typed_objects(place, &head, Kept::Lookups(lookups))?;
                let rewritten = rewritten(members, &since, &objects);
                let differing = self.differing(place, &rewritten);
                Some(Fallback {
                    objects: objects
                        .into_iter()
                        .map(|(path, (_, object))| (path, object))
                        .collect(),
                    staged: self.changed_in_index(place, &head)?,
                    differing,
                })
            }
            None => None,
        };
        Ok(RepositoryFacts {
            known: listing.known,
            uncommitted: listing.uncommitted,
            since,
            fallback,
        })
    }

    /// The files that differ under the anchor of each of the `rewritten`
    /// elements of the repository at `place`, keyed by the anchor and the
    /// object its stamp recorded. A file anchor lists itself. A folder
    /// anchor lists the files that differ between the recorded tree and
    /// HEAD's once Git confirmed the recorded object as a tree in this
    /// repository; each pair of trees is compared once. An element whose
    /// files cannot be told, because Git does not confirm the tree, fails,
    /// or prints a path that does not fit, or because no file differs, is
    /// left out, so that review flags it as a stamp whose commit is unknown.
    fn differing(
        &self,
        place: &Place,
        rewritten: &BTreeSet<Rewritten<'_>>,
    ) -> BTreeMap<(Vec<u8>, ObjectId), BTreeSet<Vec<u8>>> {
        let candidates: Vec<&ObjectId> = rewritten
            .iter()
            .filter_map(|element| match element {
                Rewritten::Trees { recorded, .. } => Some(*recorded),
                Rewritten::File { .. } => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let confirmed = self.confirmed_trees(place, &candidates);
        let diffs: BTreeMap<(&ObjectId, &ObjectId), Vec<Vec<u8>>> = rewritten
            .iter()
            .filter_map(|element| match element {
                Rewritten::Trees { recorded, head, .. } if confirmed.contains(*recorded) => {
                    Some((*recorded, *head))
                }
                Rewritten::Trees { .. } | Rewritten::File { .. } => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|(recorded, head)| {
                Some(((recorded, head), self.tree_diff(place, recorded, head)?))
            })
            .collect();
        rewritten
            .iter()
            .filter_map(|element| match element {
                Rewritten::File { anchor, recorded } => Some((
                    (anchor.to_vec(), (*recorded).clone()),
                    BTreeSet::from([anchor.to_vec()]),
                )),
                Rewritten::Trees {
                    anchor,
                    recorded,
                    head,
                } => diffs.get(&(*recorded, *head)).map(|files| {
                    (
                        (anchor.to_vec(), (*recorded).clone()),
                        files
                            .iter()
                            .map(|file| [*anchor, b"/", file].concat())
                            .collect(),
                    )
                }),
            })
            .collect()
    }

    /// Those of `candidates` that the repository at `place` holds as trees,
    /// asked through `cat-file --batch-check` on standard input; none when
    /// Git fails or prints anything but one line per candidate.
    fn confirmed_trees(&self, place: &Place, candidates: &[&ObjectId]) -> BTreeSet<ObjectId> {
        if candidates.is_empty() {
            return BTreeSet::new();
        }
        let input: String = candidates
            .iter()
            .map(|object| format!("{object}\n"))
            .collect::<Vec<_>>()
            .concat();
        self.run_with_input(place, &["cat-file", "--batch-check"], input.as_bytes())
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| confirmed(candidates, &output.stdout))
            .unwrap_or_default()
    }

    /// The paths, relative to the trees, that differ between the trees
    /// `recorded` and `head` in the repository at `place`; `None` when Git
    /// fails, prints a path that does not fit, or names no path.
    fn tree_diff(
        &self,
        place: &Place,
        recorded: &ObjectId,
        head: &ObjectId,
    ) -> Option<Vec<Vec<u8>>> {
        let output = self
            .success(
                place,
                &[
                    "diff-tree",
                    "-r",
                    "-z",
                    "--name-only",
                    "--no-renames",
                    recorded.as_str(),
                    head.as_str(),
                    "--",
                ],
            )
            .ok()?;
        diff_paths(&output)
    }

    /// The commit `revision` names, or `None` when it names none, such as
    /// HEAD in a repository without commits.
    fn commit(&self, place: &Place, revision: &str) -> Result<Option<CommitId>, Unavailable> {
        let output = self.run(
            place,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{revision}^{{commit}}"),
            ],
        )?;
        match output.status.code() {
            Some(0) => output
                .stdout
                .strip_suffix(b"\n")
                .and_then(|id| String::from_utf8(id.to_vec()).ok())
                .and_then(|id| CommitId::try_from(id).ok())
                .map(Some)
                .ok_or(Unavailable::GitFailed),
            Some(1) => Ok(None),
            _ => Err(Unavailable::GitFailed),
        }
    }

    /// What the stamp's commit tells: the paths in which the index differs
    /// from it, or that it is unknown or no ancestor of `head`.
    fn since(
        &self,
        place: &Place,
        stamp: &CommitId,
        head: Option<&CommitId>,
    ) -> Result<Since, Unavailable> {
        if head.is_none() || !self.knows(place, stamp)? || !self.is_ancestor(place, stamp)? {
            return Ok(Since::Unknown);
        }
        Ok(Since::Changed(self.changed_in_index(place, stamp)?))
    }

    /// The paths in which the index differs from `commit`, gitlinks
    /// included whatever a `.gitmodules` file says to ignore.
    fn changed_in_index(
        &self,
        place: &Place,
        commit: &CommitId,
    ) -> Result<BTreeSet<Vec<u8>>, Unavailable> {
        let changed = self.success(
            place,
            &[
                "diff-index",
                "--cached",
                "--name-only",
                "-z",
                "--no-renames",
                "--ignore-submodules=dirty",
                "--relative",
                commit.as_str(),
                "--",
            ],
        )?;
        place.paths(records(&changed)?)
    }

    /// The paths `git status` reports, relative to the project root, where
    /// Git prints them with `prefix` in front.
    fn status(&self, place: &Place, prefix: &[u8]) -> Result<Status, Unavailable> {
        let status = self.success(
            place,
            &[
                "status",
                "--porcelain=v2",
                "-z",
                "--untracked-files=all",
                "--no-renames",
                "--ignore-submodules=dirty",
                "--no-show-stash",
                "--",
                ".",
            ],
        )?;
        let Status {
            uncommitted,
            untracked,
            repositories,
        } = status_paths(&status, prefix)?;
        Ok(Status {
            uncommitted: place.paths(uncommitted.iter().map(Vec::as_slice))?,
            untracked: place.paths(untracked.iter().map(Vec::as_slice))?,
            repositories: place.paths(repositories.iter().map(Vec::as_slice))?,
        })
    }

    /// Whether the repository holds the commit `stamp`: only then does Git
    /// print exactly that commit. Anything else, such as a tree or an
    /// annotated tag with this ID, counts as unknown.
    fn knows(&self, place: &Place, stamp: &CommitId) -> Result<bool, Unavailable> {
        let output = self.run(
            place,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{stamp}^{{commit}}"),
            ],
        )?;
        Ok(output.status.success()
            && output.stdout.strip_suffix(b"\n") == Some(stamp.as_str().as_bytes()))
    }

    /// Whether `stamp` is an ancestor of HEAD; any answer but yes or no
    /// fails the review.
    fn is_ancestor(&self, place: &Place, stamp: &CommitId) -> Result<bool, Unavailable> {
        let output = self.run(
            place,
            &["merge-base", "--is-ancestor", stamp.as_str(), "HEAD"],
        )?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(Unavailable::GitFailed),
        }
    }
}

/// The roots among `repositories` in which an element's anchor lies.
fn wanted<'a>(
    repositories: &'a BTreeSet<Vec<u8>>,
    elements: &[AnchorAndStamp<'_>],
) -> Vec<&'a Vec<u8>> {
    repositories
        .iter()
        .filter(|root| {
            elements
                .iter()
                .any(|element| element.anchor.is_some_and(|anchor| anchor.lies_in(root)))
        })
        .collect()
}

/// The elements whose repository is the one at `root`, or the project's
/// for `None`, as [`belongs_to`] decides it.
fn members<'e, 'a>(
    root: Option<&[u8]>,
    nested: &BTreeMap<Vec<u8>, RepositoryFacts>,
    elements: &'e [AnchorAndStamp<'a>],
) -> Vec<&'e AnchorAndStamp<'a>> {
    elements
        .iter()
        .filter(|element| belongs_to(element.anchor, root, nested))
        .collect()
}

/// The stamps of a repository's `members`. Each commit maps to whether a
/// stamp naming it recorded content.
fn stamps<'a>(members: &[&AnchorAndStamp<'a>]) -> BTreeMap<&'a CommitId, bool> {
    members
        .iter()
        .filter_map(|element| {
            element
                .stamp
                .map(|stamp| (stamp, element.content.is_some()))
        })
        .fold(BTreeMap::new(), |mut stamps, (stamp, recorded)| {
            *stamps.entry(stamp).or_insert(false) |= recorded;
            stamps
        })
}

/// The `members` of a repository whose stamp's commit tells nothing there
/// (`since`) and whose own anchor holds another object at HEAD (`objects`)
/// than the stamp recorded for it: a file anchor whatever the recorded
/// object, a folder anchor when the recorded object has the length of
/// HEAD's. The recorded content is read only for the element's own anchor.
/// A gitlink at the anchor, an anchor HEAD does not hold, and an anchor
/// recorded as holding nothing are left out.
fn rewritten<'a>(
    members: &[&AnchorAndStamp<'a>],
    since: &BTreeMap<CommitId, Since>,
    objects: &'a BTreeMap<Vec<u8>, TreeEntry>,
) -> BTreeSet<Rewritten<'a>> {
    members
        .iter()
        .filter(|element| {
            element
                .stamp
                .is_some_and(|stamp| since.get(stamp) == Some(&Since::Unknown))
        })
        .filter_map(|element| {
            let anchor = element.anchor?;
            let recorded = element.content?.get(anchor)?.as_ref()?;
            let (path, (kind, head)) = objects.get_key_value(anchor.as_str().as_bytes())?;
            match kind {
                _ if head == recorded => None,
                ObjectKind::Blob => Some(Rewritten::File {
                    anchor: path,
                    recorded,
                }),
                ObjectKind::Tree if head.as_str().len() == recorded.as_str().len() => {
                    Some(Rewritten::Trees {
                        anchor: path,
                        recorded,
                        head,
                    })
                }
                ObjectKind::Tree | ObjectKind::Gitlink => None,
            }
        })
        .collect()
}

/// Whether an element with `anchor` belongs to the repository at `root`, or
/// the project's for `None`: an element without an anchor belongs to the
/// project's repository only, and an anchor must lie in `root` and in none
/// of the repositories `nested` below it, the ones Git was run in. The rule
/// must match the one review applies through [`repository_root`] over
/// [`GitFacts::nested`], or a stamp is measured in the wrong repository.
fn belongs_to(
    anchor: Option<&Anchor>,
    root: Option<&[u8]>,
    nested: &BTreeMap<Vec<u8>, RepositoryFacts>,
) -> bool {
    root.is_none_or(|root| anchor.is_some_and(|anchor| anchor.lies_in(root)))
        && repository_root(nested.keys(), anchor).is_none()
}

/// The settings for Git in a nested repository below `ceiling`: Git looks
/// for the repository no higher than the nested directory, runs no hook,
/// and runs no filter of the `drivers`. The configuration goes through the
/// environment, which takes every driver name as it is.
fn nested_settings(ceiling: &Path, drivers: &BTreeSet<Vec<u8>>) -> Vec<(OsString, OsString)> {
    let overrides: Vec<(Vec<u8>, &str)> = iter::once((b"core.hooksPath".to_vec(), "/dev/null"))
        .chain(drivers.iter().flat_map(|driver| {
            FILTER_OVERRIDES.map(|(key, value)| {
                (
                    [b"filter.", driver.as_slice(), b".", key.as_bytes()].concat(),
                    value,
                )
            })
        }))
        .collect();
    [
        (
            "GIT_CEILING_DIRECTORIES".into(),
            ceiling.as_os_str().to_owned(),
        ),
        (
            "GIT_CONFIG_COUNT".into(),
            overrides.len().to_string().into(),
        ),
    ]
    .into_iter()
    .chain(
        overrides
            .into_iter()
            .enumerate()
            .flat_map(|(index, (key, value))| {
                [
                    (
                        format!("GIT_CONFIG_KEY_{index}").into(),
                        OsString::from_vec(key),
                    ),
                    (format!("GIT_CONFIG_VALUE_{index}").into(), value.into()),
                ]
            }),
    )
    .collect()
}

/// The directory at `root` below the project root `project`, if every
/// segment of `root` names an entry inside a directory and every directory
/// down to it is a real directory, not a symbolic link.
fn real_directory(project: &Path, root: &[u8]) -> Option<PathBuf> {
    root.split(|byte| *byte == b'/')
        .try_fold(project.to_path_buf(), |directory, segment| {
            let directory = directory.join(OsStr::from_bytes(segment));
            (is_plain_segment(segment) && is_real_directory(&directory)).then_some(directory)
        })
}

/// Whether `path` is a directory and not a symbolic link.
fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
}

/// `path`, relative to the top of the nested repository at `root`, made
/// relative to the project root. A path that is empty, starts with `/`, or
/// has an empty, `.`, `..`, or `.git` segment is refused, so no path a
/// nested repository prints leads out of it.
fn nested_path(root: &[u8], path: &[u8]) -> Result<Vec<u8>, Unavailable> {
    if path.split(|byte| *byte == b'/').all(is_plain_segment) {
        Ok([root, b"/", path].concat())
    } else {
        Err(Unavailable::GitFailed)
    }
}

/// Whether `segment` names an entry inside a directory: it is not empty,
/// not `.` or `..`, and not Git's own directory.
const fn is_plain_segment(segment: &[u8]) -> bool {
    !matches!(segment, b"" | b"." | b".." | b".git")
}

/// The filter drivers among the `scope NUL name NUL` pairs that
/// `git config --show-scope --name-only -z` prints, for which a scope other
/// than `global` or `system` sets a key. Any other output is broken.
fn untrusted_drivers(output: &[u8]) -> Result<BTreeSet<Vec<u8>>, Unavailable> {
    records(output)?
        .chunks(2)
        .filter_map(|pair| match pair {
            [b"global" | b"system", _] => None,
            [_, name] => driver(name).transpose(),
            _ => Some(Err(Unavailable::GitFailed)),
        })
        .collect()
}

/// The driver name in the configuration key `filter.<driver>.<setting>`, or
/// `None` for a key without one.
fn driver(name: &[u8]) -> Result<Option<Vec<u8>>, Unavailable> {
    let rest = name
        .strip_prefix(b"filter.")
        .ok_or(Unavailable::GitFailed)?;
    Ok(rest
        .iter()
        .rposition(|byte| *byte == b'.')
        .and_then(|end| rest.get(..end))
        .map(<[u8]>::to_vec))
}

/// The entries `git ls-files -s -z` prints: whether each is a gitlink, and
/// its path. Any other output is broken.
fn staged(output: &[u8]) -> Result<Vec<(bool, &[u8])>, Unavailable> {
    records(output)?.into_iter().map(staged_entry).collect()
}

/// One entry of `git ls-files -s`: `<mode> <object> <stage>`, a tab, and
/// the path.
fn staged_entry(record: &[u8]) -> Result<(bool, &[u8]), Unavailable> {
    let parts: Vec<&[u8]> = record.splitn(2, |byte| *byte == b'\t').collect();
    let [fields, path] = parts.as_slice() else {
        return Err(Unavailable::GitFailed);
    };
    let fields: Vec<&[u8]> = fields.split(|byte| *byte == b' ').collect();
    match fields.as_slice() {
        [mode, object, [b'0'..=b'3']] if is_mode(mode) && is_object(object) && !path.is_empty() => {
            Ok((*mode == b"160000", path))
        }
        _ => Err(Unavailable::GitFailed),
    }
}

/// The entries `git ls-tree -r -t -z` prints: each path relative to where
/// Git ran with its object, without the folders that enclose that place,
/// `./` and a chain of `../`. A mode that does not fit its type, an object
/// that is not a full object ID, an empty path, a path with an empty, `.`,
/// `..`, or `.git` segment, or a path printed twice is broken output, so
/// that a crafted tree cannot choose which of two objects counts.
fn tree_entries(output: &[u8]) -> Result<BTreeMap<&[u8], TreeEntry>, Unavailable> {
    let entries = records(output)?
        .into_iter()
        .filter_map(|record| tree_entry(record).transpose())
        .collect::<Result<Vec<_>, Unavailable>>()?;
    let count = entries.len();
    let entries: BTreeMap<&[u8], TreeEntry> = entries.into_iter().collect();
    if entries.len() == count {
        Ok(entries)
    } else {
        Err(Unavailable::GitFailed)
    }
}

/// One entry of `git ls-tree`: `<mode> <type> <object>`, a tab, and the
/// path; `None` for a folder that encloses the place Git ran in.
fn tree_entry(record: &[u8]) -> Result<Option<(&[u8], TreeEntry)>, Unavailable> {
    let parts: Vec<&[u8]> = record.splitn(2, |byte| *byte == b'\t').collect();
    let [fields, path] = parts.as_slice() else {
        return Err(Unavailable::GitFailed);
    };
    let fields: Vec<&[u8]> = fields.split(|byte| *byte == b' ').collect();
    let [mode, kind, object] = fields.as_slice() else {
        return Err(Unavailable::GitFailed);
    };
    let kind = match (*mode, *kind) {
        (b"040000", b"tree") => Some(ObjectKind::Tree),
        (b"160000", b"commit") => Some(ObjectKind::Gitlink),
        (b"100644" | b"100755" | b"120000", b"blob") => Some(ObjectKind::Blob),
        _ => None,
    };
    let object = String::from_utf8(object.to_vec())
        .ok()
        .and_then(|object| ObjectId::try_from(object).ok());
    match (kind, object) {
        (Some(ObjectKind::Tree), Some(_)) if encloses(path) => Ok(None),
        (Some(kind), Some(object)) if path.split(|byte| *byte == b'/').all(is_plain_segment) => {
            Ok(Some((path, (kind, object))))
        }
        _ => Err(Unavailable::GitFailed),
    }
}

/// The `candidates` that `git cat-file --batch-check` confirms as trees in
/// its `output`, which holds one line per candidate in their order; `None`
/// when the output has another number of lines or does not end with a line
/// feed. A line that is not exactly the candidate's full ID, `tree`, and a
/// decimal size leaves its candidate out.
fn confirmed(candidates: &[&ObjectId], output: &[u8]) -> Option<BTreeSet<ObjectId>> {
    let lines: Vec<&[u8]> = output
        .strip_suffix(b"\n")?
        .split(|byte| *byte == b'\n')
        .collect();
    if lines.len() != candidates.len() {
        return None;
    }
    Some(
        candidates
            .iter()
            .zip(lines)
            .filter(|(candidate, line)| {
                line.strip_prefix(candidate.as_str().as_bytes())
                    .and_then(|rest| rest.strip_prefix(b" tree "))
                    .is_some_and(|size| !size.is_empty() && size.iter().all(u8::is_ascii_digit))
            })
            .map(|(candidate, _)| (*candidate).clone())
            .collect(),
    )
}

/// The paths `git diff-tree -r -z --name-only` prints between two trees,
/// relative to them; `None` when the output is not NUL-terminated, a path is
/// empty or has an empty, `.`, `..`, or `.git` segment, or no path is
/// printed.
fn diff_paths(output: &[u8]) -> Option<Vec<Vec<u8>>> {
    let paths = records(output).ok()?;
    let fits = !paths.is_empty()
        && paths
            .iter()
            .all(|path| path.split(|byte| *byte == b'/').all(is_plain_segment));
    fits.then(|| paths.iter().map(|path| path.to_vec()).collect())
}

/// The entries of `git ls-tree -r -t -z` `output`, printed in the repository
/// at `root` (the project's for `None`), with paths relative to the project
/// root, those that `kept` keeps. Every entry is checked first, as
/// [`tree_entries`] does, and its path made relative to the project root,
/// so a broken entry fails wherever it lies.
fn tree_objects(
    output: &[u8],
    root: Option<&[u8]>,
    kept: Kept<'_>,
) -> Result<BTreeMap<Vec<u8>, TreeEntry>, Unavailable> {
    let entries = tree_entries(output)?
        .into_iter()
        .map(|(path, entry)| {
            let path = root.map_or_else(|| Ok(path.to_vec()), |root| nested_path(root, path))?;
            Ok((path, entry))
        })
        .collect::<Result<Vec<_>, Unavailable>>()?;
    Ok(entries
        .into_iter()
        .filter(|(path, _)| kept.keeps(path))
        .collect())
}

/// Whether `path` names a folder that encloses the place Git ran in, as
/// `ls-tree` prints it below the top of the work tree: `./`, or one or more
/// `../`, optionally followed by `./`.
fn encloses(path: &[u8]) -> bool {
    let mut rest = path;
    while let Some(after) = rest.strip_prefix(b"../") {
        rest = after;
    }
    (rest.is_empty() && rest.len() < path.len()) || rest == b"./"
}

/// Whether `mode` is six octal digits.
fn is_mode(mode: &[u8]) -> bool {
    mode.len() == 6 && mode.iter().all(|byte| (b'0'..=b'7').contains(byte))
}

/// Whether `object` is the hex ID of an object, SHA-1 or SHA-256.
fn is_object(object: &[u8]) -> bool {
    matches!(object.len(), 40 | 64)
        && object
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

/// The paths of `git status`, relative to the project root.
#[derive(Debug, Default, PartialEq, Eq)]
struct Status {
    /// Paths whose working tree differs from the index, that are unmerged,
    /// or that are untracked; never the root of an untracked repository.
    uncommitted: BTreeSet<Vec<u8>>,
    /// Paths that are untracked and not ignored; never the root of an
    /// untracked repository.
    untracked: BTreeSet<Vec<u8>>,
    /// The roots of untracked repositories, without a final `/`.
    repositories: BTreeSet<Vec<u8>>,
}

/// The NUL-terminated records of `output`; output that does not end with
/// NUL is broken.
fn records(output: &[u8]) -> Result<Vec<&[u8]>, Unavailable> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    output
        .strip_suffix(b"\0")
        .map(|records| records.split(|byte| *byte == 0).collect())
        .ok_or(Unavailable::GitFailed)
}

/// The paths of `git status --porcelain=v2 -z --no-renames`, whose paths are
/// relative to the work tree's top, made relative to the project root at
/// `prefix`. Any record of another kind, or a path outside the project,
/// fails the review.
fn status_paths(output: &[u8], prefix: &[u8]) -> Result<Status, Unavailable> {
    records(output)?
        .into_iter()
        .try_fold(Status::default(), |mut status, record| {
            match status_record(record)? {
                StatusRecord::Changed {
                    path,
                    in_working_tree,
                } => {
                    if in_working_tree {
                        status.uncommitted.insert(below(prefix, path)?);
                    }
                }
                StatusRecord::Untracked(path) => {
                    let path = below(prefix, path)?;
                    status.uncommitted.insert(path.clone());
                    status.untracked.insert(path);
                }
                StatusRecord::Repository(root) => {
                    status.repositories.insert(below(prefix, root)?);
                }
            }
            Ok(status)
        })
}

/// One record of `git status --porcelain=v2`.
#[derive(Debug, PartialEq, Eq)]
enum StatusRecord<'a> {
    /// A tracked path that differs from HEAD, and whether its working tree
    /// differs from the index; unmerged paths always count as such.
    Changed {
        path: &'a [u8],
        in_working_tree: bool,
    },
    /// An untracked path.
    Untracked(&'a [u8]),
    /// The root of an untracked repository, which Git prints with a final
    /// `/` that this leaves out.
    Repository(&'a [u8]),
}

fn status_record(record: &[u8]) -> Result<StatusRecord<'_>, Unavailable> {
    let fields: Vec<&[u8]> = match record.first() {
        Some(b'1') => record.splitn(9, |byte| *byte == b' ').collect(),
        Some(b'u') => record.splitn(11, |byte| *byte == b' ').collect(),
        _ => vec![record],
    };
    match fields.as_slice() {
        [b"1", [_, worktree], _, _, _, _, _, _, path] => Ok(StatusRecord::Changed {
            path,
            in_working_tree: *worktree != b'.',
        }),
        [b"u", _, _, _, _, _, _, _, _, _, path] => Ok(StatusRecord::Changed {
            path,
            in_working_tree: true,
        }),
        [untracked] => untracked
            .strip_prefix(b"? ")
            .map(|path| {
                path.strip_suffix(b"/")
                    .map_or(StatusRecord::Untracked(path), StatusRecord::Repository)
            })
            .ok_or(Unavailable::GitFailed),
        _ => Err(Unavailable::GitFailed),
    }
}

/// `path`, relative to the work tree's top, made relative to the project
/// root at `prefix`.
fn below(prefix: &[u8], path: &[u8]) -> Result<Vec<u8>, Unavailable> {
    path.strip_prefix(prefix)
        .filter(|path| !path.is_empty())
        .map(<[u8]>::to_vec)
        .ok_or(Unavailable::GitFailed)
}

#[cfg(test)]
mod tests;
