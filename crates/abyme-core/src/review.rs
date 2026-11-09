//! Review flags: which elements may no longer match what they stand for.
//!
//! [`review`] computes them from the elements and from the facts that Git
//! delivers as data, without running Git, so every rule can be tested on
//! its own. Flags are never stored: they are computed again whenever they
//! are read, so they cannot go stale.

use std::collections::{BTreeMap, BTreeSet};

use abyme_model::{
    Anchor, CommitId, Element, ElementId, ElementProblem, ObjectId, RecordedContent, RelationKind,
    Verification, Verifier,
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
    /// no ancestor of HEAD there, and the files that changed since cannot be
    /// told: the stamp recorded no content, or the content it recorded for
    /// the anchor does not match HEAD and Git cannot list the files that
    /// differ from it.
    UnknownStamp,
    /// These files under the anchor differ from the stamp's commit, or after
    /// a rewrite from the content the stamp recorded, or are untracked in the
    /// element's repository: paths relative to the project root, sorted.
    Changed(Vec<Vec<u8>>),
    /// No file that the element's repository tracks or lists as untracked
    /// lies under the anchor.
    AnchorMissing,
    /// What the anchor of the element a relation of this kind points to
    /// stands for may have changed since this element's stamp: in this
    /// element's repository, files under that anchor changed, or the root of
    /// the nested repository the anchor lies in; after a rewrite, also when
    /// that path holds another object than the stamp recorded, or the stamp
    /// holds no entry for it. A path the stamp recorded as holding nothing
    /// passes the flag on as soon as it holds something.
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
/// relative to the project root, and lie below the repository's root; a
/// repository nested in it shows only as its root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryFacts {
    /// The paths that Git tracks or lists as untracked and not ignored.
    pub known: BTreeSet<Vec<u8>>,
    /// The paths whose working tree differs from the index, that are
    /// unmerged, or that are untracked.
    pub uncommitted: BTreeSet<Vec<u8>>,
    /// What each stamp commit of the repository's elements tells.
    pub since: BTreeMap<CommitId, Since>,
    /// The repository's content at HEAD, when a stamp that recorded content
    /// names a commit that tells nothing; `None` when no stamp needs it or
    /// the repository has no commit.
    pub fallback: Option<Fallback>,
}

/// The content of a repository at the commit HEAD names, by which a stamp
/// whose commit tells nothing is judged. Paths are bytes, relative to the
/// project root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fallback {
    /// The object each path held at that commit, for the paths review looks
    /// up: the anchors of the elements and the folders above them, the
    /// repository's own root included when it is nested. A path that is
    /// missing held nothing or is no such path; review looks up only the
    /// former.
    pub objects: BTreeMap<Vec<u8>, ObjectId>,
    /// The paths in which the index differs from that commit.
    pub staged: BTreeSet<Vec<u8>>,
    /// For each own anchor that holds another object at that commit than
    /// a stamp recorded, the files under it that differ, keyed by the
    /// anchor and the recorded object; missing where they cannot be told.
    pub differing: BTreeMap<(Vec<u8>, ObjectId), BTreeSet<Vec<u8>>>,
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
    /// The root of the repository, relative to the project root; `None` for
    /// the project's repository.
    pub root: Option<Vec<u8>>,
    /// The roots of the repositories nested in the project that Git showed
    /// on the way to this one, so that a relation target's path in it can be
    /// told.
    pub roots: BTreeSet<Vec<u8>>,
    /// The object each path held at that commit: every file, folder, and
    /// nested repository below the project root, and the repository's own
    /// root when it is nested. A path that is missing held nothing.
    pub objects: BTreeMap<Vec<u8>, ObjectId>,
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
                let repository = facts.repository(root);
                let baseline = baseline(element, repository);
                let flags: Vec<Flag> = own_flags(element, baseline, repository)
                    .into_iter()
                    .chain(passed_on(element, baseline, root, snapshot, facts))
                    .collect();
                (!flags.is_empty()).then(|| (id.clone(), flags))
            })
            .collect(),
    )
}

/// What an element's stamp tells about the time since it, in the element's
/// repository.
#[derive(Debug, Clone, Copy)]
enum Baseline<'a> {
    /// The element has no stamp.
    Unstamped,
    /// The paths in which the index differs from the stamp's commit.
    Since(&'a BTreeSet<Vec<u8>>),
    /// The stamp's commit tells nothing, and the stamp recorded the content
    /// it vouches for, which is compared with the repository's content at
    /// HEAD.
    Content {
        /// The content the stamp recorded.
        recorded: &'a RecordedContent,
        /// The repository's content at HEAD.
        head: &'a Fallback,
    },
    /// The repository does not know the stamp's commit, it is no ancestor
    /// of HEAD there, or the facts do not name it, and there is no recorded
    /// content to judge it by.
    Unusable,
}

/// The baseline of `element`'s stamp, from the facts of its repository.
fn baseline<'a>(element: &'a Element, facts: &'a RepositoryFacts) -> Baseline<'a> {
    element.verified().map_or(Baseline::Unstamped, |stamp| {
        match facts.since.get(&stamp.commit) {
            Some(Since::Changed(paths)) => Baseline::Since(paths),
            Some(Since::Unknown) | None => match (&stamp.content, &facts.fallback) {
                (Some(recorded), Some(head)) => Baseline::Content { recorded, head },
                (Some(_) | None, _) => Baseline::Unusable,
            },
        }
    })
}

/// The flags an element has because of what lies under its own anchor and
/// of its own stamp, whose baseline is `baseline`, from the facts of its
/// repository.
fn own_flags(element: &Element, baseline: Baseline<'_>, facts: &RepositoryFacts) -> Vec<Flag> {
    let stamp = match (baseline, element.anchor()) {
        (Baseline::Since(paths), anchor) => {
            anchor.and_then(|anchor| changed_under(anchor, paths, &facts.uncommitted))
        }
        (Baseline::Content { recorded, head }, Some(anchor)) => {
            if recorded_matches(recorded, anchor, head) {
                changed_under(anchor, &head.staged, &facts.uncommitted)
            } else {
                differing_files(recorded, anchor, head).map_or(Some(Flag::UnknownStamp), |files| {
                    let since = files.iter().chain(&head.staged).cloned().collect();
                    changed_under(anchor, &since, &facts.uncommitted)
                })
            }
        }
        (Baseline::Content { .. } | Baseline::Unstamped, None) => None,
        (Baseline::Unusable, _) => Some(Flag::UnknownStamp),
        (Baseline::Unstamped, Some(_)) => Some(Flag::NeverVerified),
    };
    let missing = element
        .anchor()
        .filter(|anchor| !facts.known.iter().any(|path| anchor.covers(path)))
        .map(|_| Flag::AnchorMissing);
    stamp.into_iter().chain(missing).collect()
}

/// Whether the content recorded for `path` equals what the path, as Git
/// prints it, holds at HEAD. `path` is an own anchor or the path that
/// stands for a target, which the Git adapter keeps in the fallback's
/// objects; a path it left out would read as holding nothing. A path the stamp did not record never
/// matches, and an object of another length than HEAD's differs.
fn recorded_matches(recorded: &RecordedContent, path: &Anchor, head: &Fallback) -> bool {
    recorded
        .get(path)
        .is_some_and(|object| object.as_ref() == head.objects.get(path.as_str().as_bytes()))
}

/// The files under `anchor` that differ between the object the stamp
/// recorded for it and the one HEAD holds there, if Git could tell them.
fn differing_files<'a>(
    recorded: &RecordedContent,
    anchor: &Anchor,
    head: &'a Fallback,
) -> Option<&'a BTreeSet<Vec<u8>>> {
    let object = recorded.get(anchor)?.as_ref()?;
    head.differing
        .get(&(anchor.as_str().as_bytes().to_vec(), object.clone()))
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
/// cannot be: only its stamp changes. The stamp records the object each
/// path it vouches for held at that commit: the element's own anchor and
/// the path that stands for each relation target that passes something on,
/// except a path that cannot be written as an anchor. An element with an
/// anchor is refused while files under the anchor differ from that commit
/// or are untracked; an element without an anchor, or whose anchor matches
/// no file, can be stamped.
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
    let root = head.root.as_deref();
    let targets = element
        .relations()
        .keys()
        .filter(|(kind, _)| passes_on(*kind).is_some())
        .filter_map(|(_, target)| snapshot.elements.get(target)?.anchor())
        .filter_map(|anchor| standing_path(anchor, root, &head.roots)?.as_anchor());
    let content: RecordedContent = element
        .anchor()
        .cloned()
        .into_iter()
        .chain(targets)
        .map(|path| {
            let object = head.objects.get(path.as_str().as_bytes()).cloned();
            (path, object)
        })
        .collect();
    let mut fields = element.clone().into_fields();
    fields.verified = Some(Verification {
        commit: commit.clone(),
        by,
        content: Some(content),
    });
    let element = Element::new(id.clone(), fields).map_err(|error| VerifyProblem::Rejected {
        element: id.clone(),
        problem: error.problem().clone(),
    })?;
    Ok((element, commit))
}

/// The path by which a change of what a relation target's anchor stands for
/// shows in the source's repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StandingPath<'a> {
    /// The target belongs to the source's repository: every path under its
    /// anchor.
    Anchor(&'a Anchor),
    /// The target lies in a repository nested in the source's: the root of
    /// that repository, which the source's repository sees as one path.
    NestedRoot(&'a [u8]),
}

impl StandingPath<'_> {
    /// Whether a change of `path` in the source's repository is a change
    /// of what the target stands for.
    fn shows_in(self, path: &[u8]) -> bool {
        match self {
            Self::Anchor(anchor) => anchor.covers(path),
            Self::NestedRoot(root) => path == root,
        }
    }

    /// The path as a stamp records it. A nested root that a valid anchor
    /// lies in is a valid anchor itself, so `None` is a guard only.
    fn as_anchor(self) -> Option<Anchor> {
        match self {
            Self::Anchor(anchor) => Some(anchor.clone()),
            Self::NestedRoot(root) => String::from_utf8(root.to_vec())
                .ok()
                .and_then(|root| Anchor::try_from(root).ok()),
        }
    }
}

/// The path that stands for a relation target with `anchor` in the
/// repository with the root `root`, the source's, among the repositories at
/// `roots` nested in the project: the anchor itself when the target belongs
/// to that repository; otherwise, when the anchor lies in that repository,
/// the root of the outermost repository nested in it that the anchor lies
/// in; and `None` for a target outside the source's repository.
fn standing_path<'a, R>(
    anchor: &'a Anchor,
    root: Option<&[u8]>,
    roots: R,
) -> Option<StandingPath<'a>>
where
    R: IntoIterator<Item = &'a Vec<u8>> + Clone,
{
    if repository_root(roots.clone(), Some(anchor)) == root {
        return Some(StandingPath::Anchor(anchor));
    }
    if root.is_some_and(|root| !anchor.lies_in(root)) {
        return None;
    }
    let depth = root.map_or(0, <[u8]>::len);
    roots
        .into_iter()
        .filter(|inner| inner.len() > depth && anchor.lies_in(inner))
        .min_by_key(|inner| inner.len())
        .map(|inner| StandingPath::NestedRoot(inner))
}

/// The flags the element's relations pass on, one hop: for each relation
/// whose kind passes something on, whether what the target's anchor stands
/// for may have changed since this element's own stamp, whose baseline is
/// `baseline`, measured in this element's repository, the one with the
/// root `root`, by the path that stands for the target there. Against
/// recorded content, the path changed when the stamp did not record it, it
/// holds another object at HEAD, or the index or the working tree changed
/// it since. Only the target's anchor counts, never the target's flags, so
/// nothing travels further.
fn passed_on(
    element: &Element,
    baseline: Baseline<'_>,
    root: Option<&[u8]>,
    snapshot: &Snapshot,
    facts: &GitFacts,
) -> Vec<Flag> {
    let (since, recorded) = match baseline {
        Baseline::Since(since) => (since, None),
        Baseline::Content { recorded, head } => (&head.staged, Some((recorded, head))),
        Baseline::Unstamped | Baseline::Unusable => return Vec::new(),
    };
    let repository = facts.repository(root);
    element
        .relations()
        .keys()
        .filter_map(|(kind, target)| {
            let priority = passes_on(*kind)?;
            let anchor = snapshot.elements.get(target)?.anchor()?;
            let standing = standing_path(anchor, root, facts.nested.keys())?;
            let content_differs = recorded.is_some_and(|(recorded, head)| {
                standing
                    .as_anchor()
                    .is_none_or(|path| !recorded_matches(recorded, &path, head))
            });
            (content_differs
                || since
                    .iter()
                    .chain(&repository.uncommitted)
                    .any(|path| standing.shows_in(path)))
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
