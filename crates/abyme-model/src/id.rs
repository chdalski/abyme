//! Readable, validated identifiers for elements and boards.
//!
//! An ID becomes a file name in `abyme/`, so the only way to get one is the
//! check in [`Id::try_from`]: lowercase ASCII letters and digits joined by
//! single hyphens, at most [`MAX_ID_LEN`] characters, and none of the names
//! Windows reserves for devices.

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::untrusted::shown;

/// The longest ID, in characters.
pub const MAX_ID_LEN: usize = 64;

/// How many characters of a rejected value an error shows.
const SHOWN_LIMIT: usize = 80;

/// What an ID identifies; it keeps element and board IDs apart.
pub trait IdKind: fmt::Debug + Clone + Copy + PartialEq + Eq + PartialOrd + Ord + Hash {
    /// The noun used in messages, such as "element".
    const NOUN: &'static str;
}

/// Marks the IDs of elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementKind {}

/// Marks the IDs of boards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoardKind {}

impl IdKind for ElementKind {
    const NOUN: &'static str = "element";
}

impl IdKind for BoardKind {
    const NOUN: &'static str = "board";
}

/// The ID of an element.
pub type ElementId = Id<ElementKind>;

/// The ID of a board.
pub type BoardId = Id<BoardKind>;

/// A validated ID of the kind `K`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id<K: IdKind> {
    value: String,
    kind: PhantomData<K>,
}

/// The rule a rejected ID breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdRule {
    /// The value is empty.
    Empty,
    /// The value is longer than [`MAX_ID_LEN`].
    TooLong,
    /// The value contains a character other than `a`–`z`, `0`–`9`, and `-`.
    InvalidCharacter,
    /// A hyphen starts or ends the value or follows another hyphen.
    MisplacedHyphen,
    /// Windows reserves the value as a device name, so no file can carry it.
    ReservedName,
}

/// A value that is not a valid ID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{shown} is not a valid {noun} ID: {rule}")]
pub struct IdError {
    shown: String,
    noun: &'static str,
    rule: IdRule,
}

impl<K: IdKind> Id<K> {
    /// The ID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl<K: IdKind> TryFrom<String> for Id<K> {
    type Error = IdError;

    fn try_from(value: String) -> Result<Self, IdError> {
        match broken_rule(&value) {
            Some(rule) => Err(IdError {
                shown: shown(&value, SHOWN_LIMIT),
                noun: K::NOUN,
                rule,
            }),
            None => Ok(Self {
                value,
                kind: PhantomData,
            }),
        }
    }
}

impl<K: IdKind> FromStr for Id<K> {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, IdError> {
        Self::try_from(value.to_owned())
    }
}

impl<K: IdKind> fmt::Display for Id<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.value)
    }
}

impl<K: IdKind> Serialize for Id<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}

impl<'de, K: IdKind> Deserialize<'de> for Id<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

impl IdError {
    /// The rule the value breaks.
    #[must_use]
    pub const fn rule(&self) -> IdRule {
        self.rule
    }
}

impl fmt::Display for IdRule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("it is empty"),
            Self::TooLong => write!(formatter, "it is longer than {MAX_ID_LEN} characters"),
            Self::InvalidCharacter => {
                formatter.write_str("only lowercase ASCII letters, digits, and hyphens are allowed")
            }
            Self::MisplacedHyphen => {
                formatter.write_str("a hyphen may only stand between letters or digits")
            }
            Self::ReservedName => formatter.write_str("Windows reserves this name for a device"),
        }
    }
}

/// The first rule `value` breaks, if any. The checks look at bytes, so no
/// Unicode letter or digit can pass as an ASCII one; the character check
/// comes before the length check, so the length in bytes is the length in
/// characters.
fn broken_rule(value: &str) -> Option<IdRule> {
    let allowed = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
    if value.is_empty() {
        Some(IdRule::Empty)
    } else if !value.bytes().all(allowed) {
        Some(IdRule::InvalidCharacter)
    } else if value.len() > MAX_ID_LEN {
        Some(IdRule::TooLong)
    } else if value.starts_with('-') || value.ends_with('-') || value.contains("--") {
        Some(IdRule::MisplacedHyphen)
    } else if is_reserved_device_name(value) {
        Some(IdRule::ReservedName)
    } else {
        None
    }
}

/// Whether Windows reserves `value` as a device name in every directory.
fn is_reserved_device_name(value: &str) -> bool {
    matches!(value, "con" | "prn" | "aux" | "nul")
        || matches!(
            value.as_bytes(),
            [b'c', b'o', b'm', digit] | [b'l', b'p', b't', digit] if digit.is_ascii_digit()
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::empty("", IdRule::Empty)]
    #[case::sixty_five_characters(&"a".repeat(65), IdRule::TooLong)]
    #[case::short_but_many_bytes(&"é".repeat(33), IdRule::InvalidCharacter)]
    #[case::leading_hyphen("-a", IdRule::MisplacedHyphen)]
    #[case::trailing_hyphen("a-", IdRule::MisplacedHyphen)]
    #[case::double_hyphen("a--b", IdRule::MisplacedHyphen)]
    #[case::uppercase("A", IdRule::InvalidCharacter)]
    #[case::mixed_case("aB", IdRule::InvalidCharacter)]
    #[case::underscore("a_b", IdRule::InvalidCharacter)]
    #[case::dot("a.b", IdRule::InvalidCharacter)]
    #[case::current_directory(".", IdRule::InvalidCharacter)]
    #[case::parent_directory("..", IdRule::InvalidCharacter)]
    #[case::traversal("../x", IdRule::InvalidCharacter)]
    #[case::slash("a/b", IdRule::InvalidCharacter)]
    #[case::backslash("a\\b", IdRule::InvalidCharacter)]
    #[case::space("a b", IdRule::InvalidCharacter)]
    #[case::nul_byte("a\0b", IdRule::InvalidCharacter)]
    #[case::line_break("a\n", IdRule::InvalidCharacter)]
    #[case::accented_letter("café", IdRule::InvalidCharacter)]
    #[case::cyrillic_a("\u{430}", IdRule::InvalidCharacter)]
    #[case::fullwidth_digit("\u{ff11}", IdRule::InvalidCharacter)]
    #[case::kelvin_sign("\u{212a}", IdRule::InvalidCharacter)]
    #[case::device_con("con", IdRule::ReservedName)]
    #[case::device_nul("nul", IdRule::ReservedName)]
    #[case::device_com1("com1", IdRule::ReservedName)]
    #[case::device_lpt9("lpt9", IdRule::ReservedName)]
    fn rejects_invalid_ids(#[case] value: &str, #[case] rule: IdRule) {
        let error = value.parse::<ElementId>().unwrap_err();

        assert_eq!(error.rule(), rule);
    }

    #[rstest]
    #[case::single_letter("a")]
    #[case::single_digit("0")]
    #[case::hyphenated_words("a-b-c")]
    #[case::letters_and_digits("a1-2b")]
    #[case::sixty_four_characters(&"a".repeat(64))]
    #[case::device_name_with_suffix("con-2")]
    #[case::word_starting_like_a_device("console")]
    fn accepts_valid_ids(#[case] value: &str) {
        let id = value.parse::<BoardId>().unwrap();

        assert_eq!(id.as_str(), value);
    }

    #[test]
    fn error_shows_the_value_escaped_and_names_the_kind() {
        let error = "a\nb".parse::<BoardId>().unwrap_err();

        assert_eq!(
            error.to_string(),
            r#""a\nb" is not a valid board ID: only lowercase ASCII letters, digits, and hyphens are allowed"#
        );
    }

    #[test]
    fn error_cuts_a_long_value() {
        let error = "x".repeat(1000).parse::<ElementId>().unwrap_err();

        assert!(
            error
                .to_string()
                .starts_with(&format!("\"{}…\"", "x".repeat(80)))
        );
    }

    #[test]
    fn deserializing_runs_the_same_check() {
        let result = serde_json::from_str::<ElementId>(r#""../evil""#);

        assert!(result.is_err());
    }
}
