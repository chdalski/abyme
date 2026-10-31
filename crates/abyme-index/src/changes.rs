//! What a refresh must change, decided without touching the index.

use std::collections::BTreeMap;

use abyme_model::ElementId;

/// What the index holds for one element besides its vector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The text it embeds and searches.
    pub text: String,
    /// The name of the element's statement kind, if it has one. A name, so
    /// that a stored value no kind has differs from every wanted one, and
    /// the next refresh replaces its row.
    pub statement: Option<String>,
}

/// The rows a refresh removes and the rows it embeds and adds.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Stored rows whose element is gone or whose row changed.
    pub remove: Vec<ElementId>,
    /// Elements that are new or whose row changed, with their row.
    pub add: Vec<(ElementId, Row)>,
}

/// Compares the stored rows with the wanted ones. An element whose text and
/// statement kind are unchanged is neither removed nor embedded again.
pub fn changes(stored: &BTreeMap<ElementId, Row>, wanted: &BTreeMap<ElementId, Row>) -> Changes {
    let remove = stored
        .iter()
        .filter(|(id, row)| wanted.get(*id) != Some(*row))
        .map(|(id, _)| id.clone())
        .collect();
    let add = wanted
        .iter()
        .filter(|(id, row)| stored.get(*id) != Some(*row))
        .map(|(id, row)| (id.clone(), row.clone()))
        .collect();
    Changes { remove, add }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn row(text: &str, statement: Option<&str>) -> Row {
        Row {
            text: text.to_owned(),
            statement: statement.map(str::to_owned),
        }
    }

    fn texts(entries: &[(&str, &str)]) -> BTreeMap<ElementId, Row> {
        entries
            .iter()
            .map(|(id, text)| (id.parse().unwrap(), row(text, None)))
            .collect()
    }

    #[test]
    fn removes_gone_and_changed_and_adds_new_and_changed() {
        let stored = texts(&[("a", "same"), ("b", "old"), ("c", "gone")]);
        let wanted = texts(&[("a", "same"), ("b", "new"), ("d", "fresh")]);

        let found = changes(&stored, &wanted);

        assert_eq!(
            found.remove,
            ["b", "c"].map(|id| id.parse::<ElementId>().unwrap())
        );
        assert_eq!(
            found.add,
            [("b", "new"), ("d", "fresh")].map(|(id, text)| (id.parse().unwrap(), row(text, None)))
        );
    }

    #[rstest]
    #[case::a_kind_comes(None, Some("idea"))]
    #[case::a_kind_goes(Some("idea"), None)]
    #[case::the_kind_changes(Some("idea"), Some("current"))]
    #[case::a_stored_value_no_kind_has(Some("fact"), Some("idea"))]
    #[case::an_empty_stored_value(Some(""), None)]
    fn replaces_a_row_whose_kind_alone_changed(
        #[case] stored: Option<&str>,
        #[case] wanted: Option<&str>,
    ) {
        let id: ElementId = "a".parse().unwrap();
        let stored = BTreeMap::from([(id.clone(), row("same", stored))]);
        let wanted = BTreeMap::from([(id.clone(), row("same", wanted))]);

        let found = changes(&stored, &wanted);

        assert_eq!(found.remove, std::slice::from_ref(&id));
        assert_eq!(found.add, [(id.clone(), wanted[&id].clone())]);
    }

    fn rows() -> impl Strategy<Value = BTreeMap<ElementId, Row>> {
        prop::collection::btree_map(
            prop::sample::select(vec!["a", "b", "c", "d", "e"])
                .prop_map(|id| id.parse::<ElementId>().unwrap()),
            (
                prop::sample::select(vec!["x", "y", "z"]),
                prop::option::of(prop::sample::select(vec!["current", "idea"])),
            )
                .prop_map(|(text, statement)| row(text, statement)),
            0..5,
        )
    }

    proptest! {
        #[test]
        fn applying_the_changes_yields_the_wanted_rows(
            stored in rows(),
            wanted in rows(),
        ) {
            let found = changes(&stored, &wanted);

            let mut result = stored.clone();
            for id in &found.remove {
                result.remove(id);
            }
            for (id, row) in &found.add {
                prop_assert!(result.insert(id.clone(), row.clone()).is_none());
            }
            prop_assert_eq!(&result, &wanted);
            for (id, _) in &found.add {
                prop_assert_ne!(stored.get(id), wanted.get(id));
            }
        }
    }
}
