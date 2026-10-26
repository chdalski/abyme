//! The anchor: the path in the project that an element stands for.

use std::fmt;

use crate::element::{ElementProblem, is_allowed};
use crate::untrusted::{hides_or_breaks_text, shown};

/// The longest anchor, in bytes.
pub const MAX_ANCHOR_BYTES: usize = 4096;

/// The longest segment of an anchor, in bytes.
pub const MAX_SEGMENT_BYTES: usize = 255;

/// How many characters of a rejected anchor an error shows.
const SHOWN_LIMIT: usize = 200;

/// A path relative to the project root that an element stands for, such as
/// `crates/payment-service`: a file or a folder and everything below it.
///
/// An anchor is built only through its checks and kept as it was given,
/// never normalized; a later comparison is byte for byte.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Anchor(String);

/// The rule a rejected anchor breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorRule {
    /// The anchor is empty.
    Empty,
    /// The anchor is longer than [`MAX_ANCHOR_BYTES`].
    TooLong,
    /// A segment is longer than [`MAX_SEGMENT_BYTES`].
    SegmentTooLong,
    /// The anchor starts with `/`.
    LeadingSlash,
    /// The anchor ends with `/`.
    TrailingSlash,
    /// A segment is empty.
    EmptySegment,
    /// A segment is `.` or `..`.
    DotSegment,
    /// The anchor holds a backslash.
    Backslash,
    /// The anchor holds a colon.
    Colon,
    /// The anchor holds a character that a reader cannot see, that breaks a
    /// line or reorders text, or that an element file cannot hold.
    Character(u32),
    /// The anchor lies in `abyme` or `.abyme` at the project root, or in a
    /// `.git` folder.
    Reserved,
}

impl Anchor {
    /// The anchor as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Anchor {
    type Error = ElementProblem;

    fn try_from(value: String) -> Result<Self, ElementProblem> {
        match broken_rule(&value) {
            Some(rule) => Err(ElementProblem::InvalidAnchor {
                anchor: shown(&value, SHOWN_LIMIT),
                rule,
            }),
            None => Ok(Self(value)),
        }
    }
}

impl fmt::Display for Anchor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for AnchorRule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("is empty"),
            Self::TooLong => write!(formatter, "is longer than {MAX_ANCHOR_BYTES} bytes"),
            Self::SegmentTooLong => {
                write!(formatter, "has a segment longer than {MAX_SEGMENT_BYTES} bytes")
            }
            Self::LeadingSlash => {
                formatter.write_str("starts with `/`; an anchor is relative to the project root")
            }
            Self::TrailingSlash => formatter
                .write_str("ends with `/`; an anchor covers everything below it without one"),
            Self::EmptySegment => formatter.write_str("has an empty segment"),
            Self::DotSegment => formatter.write_str("has a `.` or `..` segment"),
            Self::Backslash => {
                formatter.write_str("holds a backslash; segments are separated by `/`")
            }
            Self::Colon => formatter.write_str("holds a colon"),
            Self::Character(code) => write!(
                formatter,
                "holds U+{code:04X}, which a reader cannot see or which an element file cannot hold"
            ),
            Self::Reserved => formatter.write_str(
                "lies in `abyme` or `.abyme` at the project root or in a `.git` folder, which are reserved",
            ),
        }
    }
}

/// The first rule `anchor` breaks, if any. The length is checked first, so
/// nothing longer than [`MAX_ANCHOR_BYTES`] is scanned further.
fn broken_rule(anchor: &str) -> Option<AnchorRule> {
    if anchor.is_empty() {
        return Some(AnchorRule::Empty);
    }
    if anchor.len() > MAX_ANCHOR_BYTES {
        return Some(AnchorRule::TooLong);
    }
    if let Some(character) = anchor
        .chars()
        .find(|character| hides_or_breaks_text(*character) || !is_allowed(*character))
    {
        return Some(AnchorRule::Character(u32::from(character)));
    }
    if anchor.contains('\\') {
        return Some(AnchorRule::Backslash);
    }
    if anchor.contains(':') {
        return Some(AnchorRule::Colon);
    }
    if anchor.starts_with('/') {
        return Some(AnchorRule::LeadingSlash);
    }
    if anchor.ends_with('/') {
        return Some(AnchorRule::TrailingSlash);
    }
    let mut segments = anchor.split('/');
    let first = segments.next().unwrap_or_default();
    if first.eq_ignore_ascii_case("abyme") || first.eq_ignore_ascii_case(".abyme") {
        return Some(AnchorRule::Reserved);
    }
    std::iter::once(first)
        .chain(segments)
        .find_map(broken_segment_rule)
}

/// The rule a segment breaks, if any.
fn broken_segment_rule(segment: &str) -> Option<AnchorRule> {
    if segment.is_empty() {
        Some(AnchorRule::EmptySegment)
    } else if segment == "." || segment == ".." {
        Some(AnchorRule::DotSegment)
    } else if segment.len() > MAX_SEGMENT_BYTES {
        Some(AnchorRule::SegmentTooLong)
    } else if segment.eq_ignore_ascii_case(".git") {
        Some(AnchorRule::Reserved)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
