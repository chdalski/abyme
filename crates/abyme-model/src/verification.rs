//! The verification stamp: the commit an element was last checked against,
//! who checked it, and the content of each path it vouches for.

use std::collections::BTreeMap;
use std::fmt;

use crate::anchor::Anchor;
use crate::element::ElementProblem;
use crate::untrusted::shown;

/// How many characters of a rejected value an error shows.
const SHOWN_LIMIT: usize = 80;

/// A full commit ID: exactly 40 (SHA-1) or 64 (SHA-256) lowercase hex
/// digits, so that it can never be read as anything but an object ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitId(String);

/// A full Git object ID, such as that of a tree or a file: exactly 40
/// (SHA-1) or 64 (SHA-256) lowercase hex digits, like a [`CommitId`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(String);

/// The content a stamp vouches for: for each path, the object it held at
/// the stamp's commit, or `None` when nothing lay there.
pub type RecordedContent = BTreeMap<Anchor, Option<ObjectId>>;

/// Who verified an element. A label, not a checked identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verifier {
    /// The AI, through Abyme's tools.
    Ai,
    /// A person.
    Person,
}

/// A verification stamp: the element was checked against `commit` by `by`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    /// The commit the element was checked against.
    pub commit: CommitId,
    /// Who checked it.
    pub by: Verifier,
    /// The content of the paths the stamp vouches for at `commit`, or
    /// `None` when the stamp recorded none; an empty map records that it
    /// vouches for no path.
    pub content: Option<RecordedContent>,
}

/// Whether `value` is a full object ID: exactly 40 or 64 lowercase hex
/// digits.
fn is_full_object_id(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        && matches!(value.len(), 40 | 64)
}

impl CommitId {
    /// The commit ID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommitId {
    type Error = ElementProblem;

    fn try_from(value: String) -> Result<Self, ElementProblem> {
        if is_full_object_id(&value) {
            Ok(Self(value))
        } else {
            Err(ElementProblem::InvalidCommit {
                commit: shown(&value, SHOWN_LIMIT),
            })
        }
    }
}

impl ObjectId {
    /// The object ID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ObjectId {
    type Error = ElementProblem;

    fn try_from(value: String) -> Result<Self, ElementProblem> {
        if is_full_object_id(&value) {
            Ok(Self(value))
        } else {
            Err(ElementProblem::InvalidObject {
                object: shown(&value, SHOWN_LIMIT),
            })
        }
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Verifier {
    /// The verifier's name, as an element file writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ai => "ai",
            Self::Person => "person",
        }
    }

    /// The verifier with exactly this name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ai" => Some(Self::Ai),
            "person" => Some(Self::Person),
            _ => None,
        }
    }
}

impl fmt::Display for Verifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The stamp an element file's `verified`, `verified_by`, and
/// `verified_content` give: the first two both or neither, and the content
/// only with them.
///
/// # Errors
///
/// Returns an [`ElementProblem`] when only one of `verified` and
/// `verified_by` is present, content comes without either, the commit is
/// not a full commit ID, or the verifier is unknown.
pub fn stamp(
    verified: Option<String>,
    verified_by: Option<String>,
    content: Option<RecordedContent>,
) -> Result<Option<Verification>, ElementProblem> {
    match (verified, verified_by) {
        (None, None) if content.is_some() => Err(ElementProblem::ContentWithoutStamp),
        (None, None) => Ok(None),
        (Some(_), None) => Err(ElementProblem::IncompleteStamp {
            missing: "verified_by",
        }),
        (None, Some(_)) => Err(ElementProblem::IncompleteStamp {
            missing: "verified",
        }),
        (Some(commit), Some(by)) => {
            let commit = CommitId::try_from(commit)?;
            let by = Verifier::from_name(&by).ok_or_else(|| ElementProblem::UnknownVerifier {
                verifier: shown(&by, SHOWN_LIMIT),
            })?;
            Ok(Some(Verification {
                commit,
                by,
                content,
            }))
        }
    }
}

#[cfg(test)]
mod tests;
