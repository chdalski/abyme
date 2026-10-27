//! Review flags: which elements may no longer match what they stand for.
//!
//! [`review`] computes them from the elements and from the facts that Git
//! delivers as data, without running Git, so every rule can be tested on
//! its own. Flags are never stored: they are computed again whenever they
//! are read, so they cannot go stale.

use std::collections::{BTreeMap, BTreeSet};

use abyme_model::{Anchor, CommitId, Element, ElementId, RelationKind};

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
    /// Git does not know the stamp's commit, or it is no ancestor of HEAD.
    UnknownStamp,
    /// These files under the anchor differ from the stamp's commit or are
    /// untracked: paths relative to the project root, sorted.
    Changed(Vec<Vec<u8>>),
    /// No file that Git tracks or lists as untracked lies under the anchor.
    AnchorMissing,
    /// Files under the anchor of the element a relation of this kind points
    /// to differ from this element's stamp commit or are untracked.
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

/// What Git reports about the project's files. Paths are bytes, relative
/// to the project root, and lie inside the project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitFacts {
    /// The paths that Git tracks or lists as untracked and not ignored.
    pub known: BTreeSet<Vec<u8>>,
    /// The paths whose working tree differs from the index, that are
    /// unmerged, or that are untracked.
    pub uncommitted: BTreeSet<Vec<u8>>,
    /// What each stamp commit of the project tells.
    pub since: BTreeMap<CommitId, Since>,
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

    /// The highest priority among the flags of the element `id`.
    #[must_use]
    pub fn priority(&self, id: &ElementId) -> Option<Priority> {
        self.flags(id).iter().map(Flag::priority).max()
    }
}

/// The flags of every element of `snapshot` from what Git reports.
pub fn review(snapshot: &Snapshot, facts: &GitFacts) -> Review {
    Review(
        snapshot
            .elements
            .iter()
            .filter_map(|(id, element)| {
                let flags: Vec<Flag> = own_flags(element, facts)
                    .into_iter()
                    .chain(passed_on(element, snapshot, facts))
                    .collect();
                (!flags.is_empty()).then(|| (id.clone(), flags))
            })
            .collect(),
    )
}

/// The flags an element has because of what lies under its own anchor and
/// of its own stamp.
fn own_flags(element: &Element, facts: &GitFacts) -> Vec<Flag> {
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

/// The flags the element's relations pass on, one hop: for each relation
/// whose kind passes something on, whether files under the target's anchor
/// changed since this element's own stamp. Only the target's anchor counts,
/// never the target's flags, so nothing travels further.
fn passed_on(element: &Element, snapshot: &Snapshot, facts: &GitFacts) -> Vec<Flag> {
    let Some(Since::Changed(since)) = element
        .verified()
        .and_then(|stamp| facts.since.get(&stamp.commit))
    else {
        return Vec::new();
    };
    element
        .relations()
        .keys()
        .filter_map(|(kind, target)| {
            let priority = passes_on(*kind)?;
            let anchor = snapshot.elements.get(target)?.anchor()?;
            since
                .iter()
                .chain(&facts.uncommitted)
                .any(|path| anchor.covers(path))
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
