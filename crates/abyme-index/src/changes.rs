//! What a refresh must change, decided without touching the index.

use std::collections::BTreeMap;

use abyme_model::ElementId;

/// The rows a refresh removes and the texts it embeds and adds.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Stored rows whose element is gone or whose text changed.
    pub remove: Vec<ElementId>,
    /// Elements that are new or whose text changed, with their text.
    pub add: Vec<(ElementId, String)>,
}

/// Compares the stored texts with the wanted ones. An element whose text is
/// unchanged is neither removed nor embedded again.
pub fn changes(
    stored: &BTreeMap<ElementId, String>,
    wanted: &BTreeMap<ElementId, String>,
) -> Changes {
    let remove = stored
        .iter()
        .filter(|(id, text)| wanted.get(*id) != Some(*text))
        .map(|(id, _)| id.clone())
        .collect();
    let add = wanted
        .iter()
        .filter(|(id, text)| stored.get(*id) != Some(*text))
        .map(|(id, text)| (id.clone(), text.clone()))
        .collect();
    Changes { remove, add }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn texts(entries: &[(&str, &str)]) -> BTreeMap<ElementId, String> {
        entries
            .iter()
            .map(|(id, text)| (id.parse().unwrap(), (*text).to_owned()))
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
            [("b", "new"), ("d", "fresh")].map(|(id, text)| (id.parse().unwrap(), text.to_owned()))
        );
    }

    fn id_texts() -> impl Strategy<Value = BTreeMap<ElementId, String>> {
        prop::collection::btree_map(
            prop::sample::select(vec!["a", "b", "c", "d", "e"])
                .prop_map(|id| id.parse::<ElementId>().unwrap()),
            prop::sample::select(vec!["x", "y", "z"]).prop_map(str::to_owned),
            0..5,
        )
    }

    proptest! {
        #[test]
        fn applying_the_changes_yields_the_wanted_texts(
            stored in id_texts(),
            wanted in id_texts(),
        ) {
            let found = changes(&stored, &wanted);

            let mut result = stored.clone();
            for id in &found.remove {
                result.remove(id);
            }
            for (id, text) in &found.add {
                prop_assert!(result.insert(id.clone(), text.clone()).is_none());
            }
            prop_assert_eq!(&result, &wanted);
            for (id, _) in &found.add {
                prop_assert_ne!(stored.get(id), wanted.get(id));
            }
        }
    }
}
