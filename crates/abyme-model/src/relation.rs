//! The kinds of relations between elements.

use std::fmt;

use serde::{Deserialize, Serialize};

/// What a relation from one element to another says. The set is closed,
/// so that every rule that depends on the kind names each kind.
///
/// The kinds are declared in the order of their names, so relations sort
/// by the kind's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum RelationKind {
    /// A note, idea, or question refers to the target.
    Concerns,
    /// The source works only with the target.
    DependsOn,
    /// A document describes the target.
    Describes,
    /// A loose reference to the target.
    SeeAlso,
    /// The source uses the target at runtime, for example calls it.
    Uses,
}

impl RelationKind {
    /// The kind's name, as files and requests write it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Concerns => "concerns",
            Self::DependsOn => "depends_on",
            Self::Describes => "describes",
            Self::SeeAlso => "see_also",
            Self::Uses => "uses",
        }
    }

    /// The kind with exactly this name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "concerns" => Some(Self::Concerns),
            "depends_on" => Some(Self::DependsOn),
            "describes" => Some(Self::Describes),
            "see_also" => Some(Self::SeeAlso),
            "uses" => Some(Self::Uses),
            _ => None,
        }
    }
}

impl fmt::Display for RelationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    const ALL: [RelationKind; 5] = [
        RelationKind::Concerns,
        RelationKind::DependsOn,
        RelationKind::Describes,
        RelationKind::SeeAlso,
        RelationKind::Uses,
    ];

    #[test]
    fn reads_back_every_name_and_uses_it_in_requests() {
        for kind in ALL {
            assert_eq!(RelationKind::from_name(kind.as_str()), Some(kind));
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::Value::from(kind.as_str())
            );
        }
    }

    #[test]
    fn sorts_by_name() {
        let mut names: Vec<&str> = ALL.iter().map(|kind| kind.as_str()).collect();
        names.sort_unstable();

        let sorted: Vec<&str> = ALL.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(sorted, names);
        assert!(ALL.windows(2).all(|pair| pair.first() < pair.get(1)));
    }

    #[rstest]
    #[case::capitalized("Uses")]
    #[case::trailing_space("uses ")]
    #[case::hyphen("depends-on")]
    #[case::not_in_the_mvp("contains")]
    #[case::empty("")]
    fn knows_no_other_name(#[case] name: &str) {
        assert_eq!(RelationKind::from_name(name), None);
        assert!(serde_json::from_value::<RelationKind>(serde_json::Value::from(name)).is_err());
    }
}
