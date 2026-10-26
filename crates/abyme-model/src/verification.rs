//! The verification stamp: the commit an element was last checked against,
//! and who checked it.

use std::fmt;

use crate::element::ElementProblem;
use crate::untrusted::shown;

/// How many characters of a rejected value an error shows.
const SHOWN_LIMIT: usize = 80;

/// A full commit ID: exactly 40 (SHA-1) or 64 (SHA-256) lowercase hex
/// digits, so that it can never be read as anything but an object ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitId(String);

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
        let lower_hex = value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        if lower_hex && matches!(value.len(), 40 | 64) {
            Ok(Self(value))
        } else {
            Err(ElementProblem::InvalidCommit {
                commit: shown(&value, SHOWN_LIMIT),
            })
        }
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

/// The stamp an element file's `verified` and `verified_by` give: both or
/// neither.
///
/// # Errors
///
/// Returns an [`ElementProblem`] when only one of them is present, the
/// commit is not a full commit ID, or the verifier is unknown.
pub fn stamp(
    verified: Option<String>,
    verified_by: Option<String>,
) -> Result<Option<Verification>, ElementProblem> {
    match (verified, verified_by) {
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
            Ok(Some(Verification { commit, by }))
        }
    }
}

#[cfg(test)]
mod tests;
