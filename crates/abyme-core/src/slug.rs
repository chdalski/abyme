//! Deriving a free ID from a name.

use std::collections::BTreeSet;

use abyme_model::{Id, IdKind, MAX_ID_LEN};

/// A slug for `name`, unused in `taken`.
///
/// ASCII letters become lowercase, ASCII letters and digits stay, every
/// other run of characters becomes one hyphen, and hyphens at the ends are
/// dropped; an empty result becomes `fallback`. A slug that is taken or
/// not a valid ID, such as a Windows device name, gets the suffix `-2`,
/// `-3`, and so on, and the base is shortened so that the whole fits in
/// [`MAX_ID_LEN`]. Returns `None` only if no candidate is free, which the
/// suffixes rule out.
pub fn free_slug<K: IdKind>(name: &str, fallback: &str, taken: &BTreeSet<Id<K>>) -> Option<Id<K>> {
    let base = base_slug(name).unwrap_or_else(|| fallback.to_owned());
    (1..=taken.len() + 2).find_map(|number| {
        let id = Id::<K>::try_from(candidate(&base, number)).ok()?;
        (!taken.contains(&id)).then_some(id)
    })
}

/// The slug of `name` before collisions are considered, if any letter or
/// digit is left. Only ASCII is lowercased, so no other character can turn
/// into an ASCII letter.
fn base_slug(name: &str) -> Option<String> {
    let mut slug = String::new();
    let mut pending_hyphen = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_hyphen && !slug.is_empty() {
                slug.push('-');
            }
            pending_hyphen = false;
            slug.push(character.to_ascii_lowercase());
        } else {
            pending_hyphen = true;
        }
    }
    (!slug.is_empty()).then_some(slug)
}

/// The `number`th candidate for `base`: the base itself, then the base with
/// `-2`, `-3`, …, each shortened to fit [`MAX_ID_LEN`].
fn candidate(base: &str, number: usize) -> String {
    let suffix = if number == 1 {
        String::new()
    } else {
        format!("-{number}")
    };
    let room = MAX_ID_LEN.saturating_sub(suffix.len());
    let shortened = base.get(..room).unwrap_or(base).trim_end_matches('-');
    format!("{shortened}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use abyme_model::{BoardId, ElementId, ElementKind};
    use rstest::rstest;

    fn element_ids(values: &[&str]) -> BTreeSet<ElementId> {
        values.iter().map(|value| value.parse().unwrap()).collect()
    }

    fn slug(name: &str, taken: &[&str]) -> String {
        free_slug::<ElementKind>(name, "element", &element_ids(taken))
            .unwrap()
            .to_string()
    }

    #[rstest]
    #[case::words("Payment Service", "payment-service")]
    #[case::punctuation_runs("A -- B!!c", "a-b-c")]
    #[case::leading_and_trailing_symbols("  (Orders DB) ", "orders-db")]
    #[case::digits_kept("Service 2", "service-2")]
    #[case::umlaut_dropped("Überweisung", "berweisung")]
    #[case::kelvin_sign_not_a_k("\u{212a}elvin", "elvin")]
    #[case::only_symbols("!!!", "element")]
    #[case::empty("", "element")]
    fn derives_slugs_from_names(#[case] name: &str, #[case] expected: &str) {
        assert_eq!(slug(name, &[]), expected);
    }

    #[rstest]
    #[case::first_collision("Foo", &["foo"], "foo-2")]
    #[case::second_collision("Foo", &["foo", "foo-2"], "foo-3")]
    #[case::device_name("con", &[], "con-2")]
    #[case::device_name_upper("COM1", &[], "com1-2")]
    #[case::device_name_aux("aux", &["aux-2"], "aux-3")]
    fn skips_taken_and_invalid_slugs(
        #[case] name: &str,
        #[case] taken: &[&str],
        #[case] expected: &str,
    ) {
        assert_eq!(slug(name, taken), expected);
    }

    #[test]
    fn shortens_a_long_name_to_the_limit() {
        let name = "a".repeat(100);

        let result = slug(&name, &[]);

        assert_eq!(result, "a".repeat(64));
    }

    #[test]
    fn shortens_a_long_name_to_fit_the_suffix() {
        let name = "a".repeat(100);
        let taken = "a".repeat(64);

        let result = slug(&name, &[&taken]);

        assert_eq!(result, format!("{}-2", "a".repeat(62)));
    }

    #[test]
    fn drops_a_hyphen_left_at_the_cut() {
        let name = format!("{} {}", "a".repeat(61), "b".repeat(10));
        let taken = format!("{}-{}", "a".repeat(61), "bb");

        let result = slug(&name, &[&taken]);

        assert_eq!(result, format!("{}-2", "a".repeat(61)));
    }

    #[test]
    fn uses_the_board_fallback() {
        let taken: BTreeSet<BoardId> = BTreeSet::new();

        let id = free_slug(" ", "board", &taken).unwrap();

        assert_eq!(id.as_str(), "board");
    }
}
