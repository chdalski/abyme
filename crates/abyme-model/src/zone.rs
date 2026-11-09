//! The zone of a board: who changes what lies on it, and what kind of
//! statement its content makes.
//!
//! Modes and statement kinds are closed sets that hold what Abyme supports
//! today; any other name is rejected. Each is read only from a JSON string,
//! because a derived enum also reads the object form (`{"free": null}`),
//! which every other reader of the file would take for something else.

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::object_only::ObjectOnly;

/// Who changes a zone, and whether it follows something outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Mode {
    /// People and the AI change it, and nothing outside steers it.
    Free,
    /// What it shows follows what it stands for, and only reconciliation, that
    /// is the AI, changes its content; people may still arrange it.
    Mirrored,
}

/// What kind of statement content makes, so that the AI weighs it
/// correctly. The kinds are declared in the order of their names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum StatementKind {
    /// Describes today's reality; counts as fact and gets checked.
    Current,
    /// Was decided, with reasons; holds until it is superseded.
    Decision,
    /// Non-binding; only a suggestion, never a fact.
    Idea,
    /// Experience and quirks, such as "tests need Docker".
    Knowledge,
    /// Open; wants an answer.
    Question,
    /// An agreed goal, not yet reality.
    Target,
}

/// The settings of a zone: its mode and the statement kind of its content.
// `deny_unknown_fields` gives the JSON schema `additionalProperties: false`;
// reading goes through `ZoneFields`, which rejects unknown fields itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Zone {
    /// Who changes the zone.
    pub mode: Mode,
    /// What kind of statement its content makes.
    pub statement: StatementKind,
}

/// A zone as it is written, read only from a JSON object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoneFields {
    mode: Mode,
    statement: StatementKind,
}

/// Reads one value of a closed set from its name, and from nothing but a
/// string.
struct NameVisitor<T: 'static> {
    /// What the value is, for messages.
    what: &'static str,
    /// Every value of the set.
    all: &'static [T],
    /// The name of a value.
    name: fn(T) -> &'static str,
}

impl Mode {
    /// Every mode.
    pub const ALL: [Self; 2] = [Self::Free, Self::Mirrored];

    /// The mode's name, as files and requests write it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Mirrored => "mirrored",
        }
    }
}

impl StatementKind {
    /// Every statement kind.
    pub const ALL: [Self; 6] = [
        Self::Current,
        Self::Decision,
        Self::Idea,
        Self::Knowledge,
        Self::Question,
        Self::Target,
    ];

    /// The kind's name, as files and requests write it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Decision => "decision",
            Self::Idea => "idea",
            Self::Knowledge => "knowledge",
            Self::Question => "question",
            Self::Target => "target",
        }
    }

    /// The kind called exactly `name`, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

impl<'de> Deserialize<'de> for Mode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(NameVisitor {
            what: "mode",
            all: &Self::ALL,
            name: Self::as_str,
        })
    }
}

impl<'de> Deserialize<'de> for StatementKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(NameVisitor {
            what: "statement kind",
            all: &Self::ALL,
            name: Self::as_str,
        })
    }
}

impl<'de> Deserialize<'de> for Zone {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ObjectOnly(ZoneFields { mode, statement }) = ObjectOnly::deserialize(deserializer)?;
        Ok(Self { mode, statement })
    }
}

impl<T: Copy> Visitor<'_> for NameVisitor<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "the name of a {} as a string", self.what)
    }

    /// The value named `name`; an unknown name is not repeated in the
    /// error, which lists the names instead.
    fn visit_str<E: de::Error>(self, name: &str) -> Result<T, E> {
        self.all
            .iter()
            .copied()
            .find(|value| (self.name)(*value) == name)
            .ok_or_else(|| {
                let names: Vec<&str> = self.all.iter().map(|value| (self.name)(*value)).collect();
                E::custom(format_args!(
                    "unknown {}, expected one of: {}",
                    self.what,
                    names.join(", ")
                ))
            })
    }
}

#[cfg(test)]
mod tests;
