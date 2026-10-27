//! What Git reports about the project's files, read through the `git`
//! command line.
//!
//! Element files are untrusted, so no anchor ever reaches Git's command
//! line: anchors are matched against the paths Git prints. Only a stamp's
//! commit does, as a hex-checked [`CommitId`] after all options. Git runs
//! without a shell, with every `GIT_` variable of the environment removed,
//! and only with commands that leave the repository and its index as they
//! are. It never uses a bare repository it finds on its own: a clone can
//! bring the files of one, whose configuration would run commands. What Git
//! prints on errors never reaches a reader.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use abyme_model::CommitId;

use crate::review::{GitFacts, HeadFacts, Since, Unavailable};

/// The environment every Git command runs with, after every `GIT_`
/// variable is removed: no index writes, no prompts, no fetches, and no
/// replaced objects.
const ENVIRONMENT: [(&str, &str); 4] = [
    ("GIT_OPTIONAL_LOCKS", "0"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_NO_LAZY_FETCH", "1"),
    ("GIT_NO_REPLACE_OBJECTS", "1"),
];

/// The `git` program, found in a directory of `PATH` given as an absolute
/// path, so a relative entry cannot pick a `git` from the project.
struct Git {
    program: PathBuf,
}

/// What Git reports about the files of the project at `root`, and about
/// each of the `stamps`.
pub fn facts(root: &Path, stamps: &BTreeSet<&CommitId>) -> Result<GitFacts, Unavailable> {
    let git = Git::find()?;
    let prefix = git.prefix(root)?;
    let tracked = paths(&git.success(root, &["ls-files", "-z", "--cached"])?)?;
    let Status {
        uncommitted,
        untracked,
    } = git.status(root, &prefix)?;
    let head = git.commit(root, "HEAD")?;
    let since = stamps
        .iter()
        .map(|stamp| Ok(((*stamp).clone(), git.since(root, stamp, head.as_ref())?)))
        .collect::<Result<BTreeMap<_, _>, Unavailable>>()?;
    Ok(GitFacts {
        known: tracked.into_iter().chain(untracked).collect(),
        uncommitted,
        since,
    })
}

/// What `verify` needs to know of the repository of the project at `root`:
/// the commit HEAD names, and the paths that differ from that commit in the
/// index or the working tree, or are untracked.
pub fn head(root: &Path) -> Result<HeadFacts, Unavailable> {
    let git = Git::find()?;
    let prefix = git.prefix(root)?;
    let Status { uncommitted, .. } = git.status(root, &prefix)?;
    let Some(commit) = git.commit(root, "HEAD")? else {
        return Ok(HeadFacts {
            commit: None,
            changed: uncommitted,
        });
    };
    let staged = git.changed_in_index(root, &commit)?;
    Ok(HeadFacts {
        commit: Some(commit),
        changed: staged.into_iter().chain(uncommitted).collect(),
    })
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

    /// Runs `git` in `root` with `arguments`.
    fn run(&self, root: &Path, arguments: &[&str]) -> Result<Output, Unavailable> {
        let mut command = Command::new(&self.program);
        command
            .current_dir(root)
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
        command.envs(ENVIRONMENT);
        command.output().map_err(|_| Unavailable::NoGit)
    }

    /// What `git` prints when it succeeds.
    fn success(&self, root: &Path, arguments: &[&str]) -> Result<Vec<u8>, Unavailable> {
        let output = self.run(root, arguments)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(Unavailable::GitFailed)
        }
    }

    /// Where the project root lies in its work tree, as Git prints paths:
    /// empty at the top, otherwise ending with `/`.
    fn prefix(&self, root: &Path) -> Result<Vec<u8>, Unavailable> {
        let output = self.run(
            root,
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

    /// The commit `revision` names, or `None` when it names none, such as
    /// HEAD in a repository without commits.
    fn commit(&self, root: &Path, revision: &str) -> Result<Option<CommitId>, Unavailable> {
        let output = self.run(
            root,
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
        root: &Path,
        stamp: &CommitId,
        head: Option<&CommitId>,
    ) -> Result<Since, Unavailable> {
        if head.is_none() || !self.knows(root, stamp)? || !self.is_ancestor(root, stamp)? {
            return Ok(Since::Unknown);
        }
        Ok(Since::Changed(self.changed_in_index(root, stamp)?))
    }

    /// The paths in which the index differs from `commit`.
    fn changed_in_index(
        &self,
        root: &Path,
        commit: &CommitId,
    ) -> Result<BTreeSet<Vec<u8>>, Unavailable> {
        let changed = self.success(
            root,
            &[
                "diff-index",
                "--cached",
                "--name-only",
                "-z",
                "--no-renames",
                "--relative",
                commit.as_str(),
                "--",
            ],
        )?;
        Ok(paths(&changed)?.into_iter().collect())
    }

    /// The paths `git status` reports, relative to the project root at
    /// `prefix` of the work tree.
    fn status(&self, root: &Path, prefix: &[u8]) -> Result<Status, Unavailable> {
        let status = self.success(
            root,
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
        status_paths(&status, prefix)
    }

    /// Whether the repository holds the commit `stamp`: only then does Git
    /// print exactly that commit. Anything else, such as a tree or an
    /// annotated tag with this ID, counts as unknown.
    fn knows(&self, root: &Path, stamp: &CommitId) -> Result<bool, Unavailable> {
        let output = self.run(
            root,
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
    fn is_ancestor(&self, root: &Path, stamp: &CommitId) -> Result<bool, Unavailable> {
        let output = self.run(
            root,
            &["merge-base", "--is-ancestor", stamp.as_str(), "HEAD"],
        )?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(Unavailable::GitFailed),
        }
    }
}

/// The paths of `git status`, relative to the project root.
#[derive(Debug, Default, PartialEq, Eq)]
struct Status {
    /// Paths whose working tree differs from the index, that are unmerged,
    /// or that are untracked.
    uncommitted: BTreeSet<Vec<u8>>,
    /// Paths that are untracked and not ignored.
    untracked: BTreeSet<Vec<u8>>,
}

/// The NUL-terminated paths Git prints with `-z`.
fn paths(output: &[u8]) -> Result<Vec<Vec<u8>>, Unavailable> {
    records(output).map(|records| records.into_iter().map(<[u8]>::to_vec).collect())
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
            .map(StatusRecord::Untracked)
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
