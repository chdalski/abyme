//! Review flags: which elements may no longer match what they stand for.
//!
//! [`review`] computes them from the elements and from the facts that Git
//! delivers as data, without running Git, so every rule can be tested on
//! its own. Flags are never stored: they are computed again whenever they
//! are read, so they cannot go stale.

use std::collections::{BTreeMap, BTreeSet};

use abyme_model::{
    Anchor, CommitId, Element, ElementId, ElementProblem, RelationKind, Verification, Verifier,
};

use crate::apply::Snapshot;

/// How urgent a review is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// Worth a look.
    Hint,
    /// To be reviewed.
    Task,
}

/// Why an element may no longer match what it stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flag {
    /// The element has an anchor but no stamp.
    NeverVerified,
    /// The element's repository does not know the stamp's commit, or it is
    /// no ancestor of HEAD there.
    UnknownStamp,
    /// These files under the anchor differ from the stamp's commit or are
    /// untracked in the element's repository: paths relative to the project
    /// root, sorted.
    Changed(Vec<Vec<u8>>),
    /// No file that the element's repository tracks or lists as untracked
    /// lies under the anchor.
    AnchorMissing,
    /// What the anchor of the element a relation of this kind points to
    /// stands for may have changed since this element's stamp: in this
    /// element's repository, files under that anchor changed, or the root of
    /// the nested repository the anchor lies in.
    Upstream {
        /// The relation's kind, which decides the priority.
        kind: RelationKind,
        /// The element the relation points to.
        target: ElementId,
        /// How urgent the flag is, as the relation's kind passes it on.
        priority: Priority,
    },
}

/// The review flags of every element that has any.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Review(BTreeMap<ElementId, Vec<Flag>>);

/// Why review flags are unavailable. The texts are fixed, so nothing that
/// Git prints reaches a reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unavailable {
    /// The project lies in no Git work tree.
    #[error("the project lies in no Git repository")]
    NoRepository,
    /// `git` is not found or cannot be started.
    #[error("git could not be run")]
    NoGit,
    /// Git ended with an error or printed something unexpected.
    #[error("git failed")]
    GitFailed,
}

/// What Git reports about the project's repository and the repositories
/// nested in it. An element is judged by the facts of its repository: the
/// deepest nested repository whose root is its anchor or lies above it, or
/// else the project's repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitFacts {
    /// The facts of the project's repository.
    pub project: RepositoryFacts,
    /// The facts of each nested repository, by its root: a path relative to
    /// the project root as Git writes it, not empty and without a trailing
    /// `/`.
    pub nested: BTreeMap<Vec<u8>, RepositoryFacts>,
}

/// What Git reports about the files of one repository. Paths are bytes,
/// relative to the project root, and lie inside the project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryFacts {
    /// The paths that Git tracks or lists as untracked and not ignored.
    pub known: BTreeSet<Vec<u8>>,
    /// The paths whose working tree differs from the index, that are
    /// unmerged, or that are untracked.
    pub uncommitted: BTreeSet<Vec<u8>>,
    /// What each stamp commit of the repository's elements tells.
    pub since: BTreeMap<CommitId, Since>,
}

/// What `verify` needs to know of the repository. Paths are bytes,
/// relative to the project root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeadFacts {
    /// The commit HEAD names; `None` in a repository without commits.
    pub commit: Option<CommitId>,
    /// The paths that differ from that commit in the index or the working
    /// tree, that are unmerged, or that are untracked; never the root of an
    /// untracked repository.
    pub changed: BTreeSet<Vec<u8>>,
}

/// Why `verify` refuses to stamp an element. The texts are fixed but for
/// the element's ID and, for an element the stamp would break, the rule it
/// would break.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyProblem {
    /// No element file has this ID.
    #[error("no element has the ID {0}")]
    UnknownElement(ElementId),
    /// The repository has no commit to stamp.
    #[error("element {0} cannot be verified: the repository has no commit yet")]
    NoCommit(ElementId),
    /// Files under the element's anchor differ from HEAD or are untracked.
    #[error(
        "element {0} cannot be verified: files under its anchor differ from HEAD or are untracked; they must be committed first"
    )]
    Uncommitted(ElementId),
    /// The element's anchor lies in a nested repository that Git may not
    /// run in, or in which Git fails.
    #[error(
        "element {0} cannot be verified: its anchor lies in a nested repository that Abyme cannot read"
    )]
    UnreadRepository(ElementId),
    /// Git cannot be asked.
    #[error("element {element} cannot be verified: {reason}")]
    Unavailable {
        /// The element to verify.
        element: ElementId,
        /// Why Git cannot be asked.
        reason: Unavailable,
    },
    /// The stamp would make the element file break a rule, such as the
    /// frontmatter's size limit.
    #[error("element {element} cannot be verified: {problem}")]
    Rejected {
        /// The element to verify.
        element: ElementId,
        /// The rule it would break.
        problem: ElementProblem,
    },
}

/// What a stamp commit tells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Since {
    /// The paths in which the index differs from the commit.
    Changed(BTreeSet<Vec<u8>>),
    /// Git does not know the commit, or it is no ancestor of HEAD.
    Unknown,
}

impl Flag {
    /// How urgent the flag is.
    #[must_use]
    pub const fn priority(&self) -> Priority {
        match self {
            Self::NeverVerified | Self::UnknownStamp | Self::Changed(_) | Self::AnchorMissing => {
                Priority::Task
            }
            Self::Upstream { priority, .. } => *priority,
        }
    }
}

impl Review {
    /// The flags of the element `id`, none when it has none.
    #[must_use]
    pub fn flags(&self, id: &ElementId) -> &[Flag] {
        self.0.get(id).map_or(&[], Vec::as_slice)
    }

    /// The elements that have flags, by ID.
    pub fn flagged(&self) -> impl Iterator<Item = &ElementId> {
        self.0.keys()
    }

    /// The highest priority among the flags of the element `id`.
    #[must_use]
    pub fn priority(&self, id: &ElementId) -> Option<Priority> {
        self.flags(id).iter().map(Flag::priority).max()
    }
}

impl GitFacts {
    /// The facts of the repository with the root `root`, as
    /// [`repository_root`] names it.
    fn repository(&self, root: Option<&[u8]>) -> &RepositoryFacts {
        root.and_then(|root| self.nested.get(root))
            .unwrap_or(&self.project)
    }
}

/// The root of the repository an element with `anchor` belongs to, among
/// the nested repositories at `roots`: the deepest one whose root is the
/// anchor or lies above it, or `None` for the project's repository, which
/// also holds an element without an anchor.
pub fn repository_root<'a>(
    roots: impl IntoIterator<Item = &'a Vec<u8>>,
    anchor: Option<&Anchor>,
) -> Option<&'a [u8]> {
    let anchor = anchor?;
    roots
        .into_iter()
        .filter(|root| anchor.lies_in(root))
        .max_by_key(|root| root.len())
        .map(Vec::as_slice)
}

/// The flags of every element of `snapshot` from what Git reports.
pub fn review(snapshot: &Snapshot, facts: &GitFacts) -> Review {
    Review(
        snapshot
            .elements
            .iter()
            .filter_map(|(id, element)| {
                let root = repository_root(facts.nested.keys(), element.anchor());
                let flags: Vec<Flag> = own_flags(element, facts.repository(root))
                    .into_iter()
                    .chain(passed_on(element, root, snapshot, facts))
                    .collect();
                (!flags.is_empty()).then(|| (id.clone(), flags))
            })
            .collect(),
    )
}

/// The flags an element has because of what lies under its own anchor and
/// of its own stamp, from the facts of its repository.
fn own_flags(element: &Element, facts: &RepositoryFacts) -> Vec<Flag> {
    let stamp = match (element.verified(), element.anchor()) {
        (Some(stamp), anchor) => match facts.since.get(&stamp.commit) {
            Some(Since::Changed(paths)) => {
                anchor.and_then(|anchor| changed_under(anchor, paths, &facts.uncommitted))
            }
            Some(Since::Unknown) | None => Some(Flag::UnknownStamp),
        },
        (None, Some(_)) => Some(Flag::NeverVerified),
        (None, None) => None,
    };
    let missing = element
        .anchor()
        .filter(|anchor| !facts.known.iter().any(|path| anchor.covers(path)))
        .map(|_| Flag::AnchorMissing);
    stamp.into_iter().chain(missing).collect()
}

/// The flag for the files under `anchor` that the index changed since the
/// stamp, or that the working tree changed, if there are any.
fn changed_under(
    anchor: &Anchor,
    since: &BTreeSet<Vec<u8>>,
    uncommitted: &BTreeSet<Vec<u8>>,
) -> Option<Flag> {
    let changed: BTreeSet<&Vec<u8>> = since
        .iter()
        .chain(uncommitted)
        .filter(|path| anchor.covers(path))
        .collect();
    (!changed.is_empty()).then(|| Flag::Changed(changed.into_iter().cloned().collect()))
}

/// The element `id` stamped by `by` with the commit HEAD names, or why it
/// cannot be: only its stamp changes. An element with an anchor is refused
/// while files under the anchor differ from that commit or are untracked;
/// an element without an anchor, or whose anchor matches no file, can be
/// stamped.
pub fn stamped(
    snapshot: &Snapshot,
    id: &ElementId,
    head: &HeadFacts,
    by: Verifier,
) -> Result<(Element, CommitId), VerifyProblem> {
    let element = snapshot
        .elements
        .get(id)
        .ok_or_else(|| VerifyProblem::UnknownElement(id.clone()))?;
    let commit = head
        .commit
        .clone()
        .ok_or_else(|| VerifyProblem::NoCommit(id.clone()))?;
    if element
        .anchor()
        .is_some_and(|anchor| head.changed.iter().any(|path| anchor.covers(path)))
    {
        return Err(VerifyProblem::Uncommitted(id.clone()));
    }
    let mut fields = element.clone().into_fields();
    fields.verified = Some(Verification {
        commit: commit.clone(),
        by,
    });
    let element = Element::new(id.clone(), fields).map_err(|error| VerifyProblem::Rejected {
        element: id.clone(),
        problem: error.problem().clone(),
    })?;
    Ok((element, commit))
}

/// The flags the element's relations pass on, one hop: for each relation
/// whose kind passes something on, whether what the target's anchor stands
/// for may have changed since this element's own stamp, measured in this
/// element's repository, the one with the root `root`. A changed path
/// counts when it lies under the target's anchor and the target belongs to
/// the same repository, or when it is the root of a nested repository the
/// target's anchor lies in. Only the target's anchor counts, never the
/// target's flags, so nothing travels further.
fn passed_on(
    element: &Element,
    root: Option<&[u8]>,
    snapshot: &Snapshot,
    facts: &GitFacts,
) -> Vec<Flag> {
    let repository = facts.repository(root);
    let Some(Since::Changed(since)) = element
        .verified()
        .and_then(|stamp| repository.since.get(&stamp.commit))
    else {
        return Vec::new();
    };
    element
        .relations()
        .keys()
        .filter_map(|(kind, target)| {
            let priority = passes_on(*kind)?;
            let anchor = snapshot.elements.get(target)?.anchor()?;
            let same_repository = repository_root(facts.nested.keys(), Some(anchor)) == root;
            since
                .iter()
                .chain(&repository.uncommitted)
                .any(|path| {
                    (same_repository && anchor.covers(path))
                        || (facts.nested.contains_key(path) && anchor.lies_in(path))
                })
                .then(|| Flag::Upstream {
                    kind: *kind,
                    target: target.clone(),
                    priority,
                })
        })
        .collect()
}

/// What a relation of this kind passes on from its target to its source.
const fn passes_on(kind: RelationKind) -> Option<Priority> {
    match kind {
        RelationKind::DependsOn | RelationKind::Describes => Some(Priority::Task),
        RelationKind::Uses | RelationKind::Concerns => Some(Priority::Hint),
        RelationKind::SeeAlso => None,
    }
}

#[cfg(test)]
mod tests;
