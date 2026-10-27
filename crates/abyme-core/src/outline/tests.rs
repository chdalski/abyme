use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use abyme_model::{
    Anchor, CommitId, ElementFields, FORMAT_AND_SEPARATORS, Rect, Relations, TypeRef, Verification,
    Verifier,
};
use proptest::prelude::*;
use rstest::rstest;

use super::*;
use crate::review::{GitFacts, Since, review};

/// The six characters or sequences that end a line somewhere.
const LINE_BREAKS: [&str; 6] = ["\n", "\r", "\r\n", "\u{85}", "\u{2028}", "\u{2029}"];

fn rect() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: NonZeroU32::MIN,
        height: NonZeroU32::MIN,
    }
}

fn element(id: &str, type_ref: &str, name: &str, summary: Option<&str>) -> Element {
    Element::new(
        id.parse().unwrap(),
        ElementFields {
            type_ref: TypeRef::try_from(type_ref.to_owned()).unwrap(),
            name: name.to_owned(),
            summary: summary.map(str::to_owned),
            body: String::new(),
            relations: Relations::new(),
            anchor: None,
            verified: None,
        },
    )
    .unwrap()
}

/// A snapshot with the board `b` titled `title` that places `elements` and
/// the IDs in `missing`, whose files do not exist.
fn snapshot(title: &str, elements: Vec<Element>, missing: &[&str]) -> Snapshot {
    let placed = elements
        .iter()
        .map(|element| element.id().clone())
        .chain(missing.iter().map(|id| id.parse().unwrap()));
    let board = Board {
        id: "b".parse().unwrap(),
        title: title.to_owned(),
        placements: placed.map(|id| (id, rect())).collect(),
    };
    Snapshot {
        elements: elements
            .into_iter()
            .map(|element| (element.id().clone(), element))
            .collect(),
        boards: BTreeMap::from([(board.id.clone(), board)]),
    }
}

fn outline_of(snapshot: &Snapshot) -> String {
    outline(snapshot, &"b".parse().unwrap(), Geometry::Hidden).unwrap()
}

#[test]
fn renders_the_documented_format() {
    let snapshot = snapshot(
        "Container Level",
        vec![
            element(
                "payment-service",
                "c4/container",
                "Payment Service",
                Some("Stripe payments"),
            ),
            element("orders-db", "c4/datastore", "Orders DB", None),
            element(
                "checkout",
                "c4/container",
                "Checkout",
                Some("order completion"),
            ),
        ],
        &["web-shop"],
    );

    assert_eq!(
        outline_of(&snapshot),
        "Board: b · Container Level\n\
         \n\
         checkout         c4/container  Checkout         order completion\n\
         orders-db        c4/datastore  Orders DB\n\
         payment-service  c4/container  Payment Service  Stripe payments\n\
         web-shop         ⚠ missing\n"
    );
}

#[test]
fn renders_a_board_without_placements_as_its_header() {
    assert_eq!(
        outline_of(&snapshot("Empty", Vec::new(), &[])),
        "Board: b · Empty\n\n"
    );
}

#[test]
fn shows_a_missing_element_without_affecting_the_others() {
    let snapshot = snapshot(
        "Board",
        vec![
            element("a", "t", "A", Some("first")),
            element("c", "t", "C", None),
        ],
        &["b"],
    );

    let outline = outline_of(&snapshot);

    assert_eq!(
        outline.lines().collect::<Vec<_>>(),
        [
            "Board: b · Board",
            "",
            "a  t  A  first",
            "b  ⚠ missing",
            "c  t  C"
        ]
    );
}

#[test]
fn names_an_unknown_board() {
    let error = outline(
        &Snapshot::default(),
        &"nowhere".parse().unwrap(),
        Geometry::Hidden,
    )
    .unwrap_err();

    assert_eq!(error.to_string(), "no board has the ID nowhere");
}

#[rstest]
fn keeps_each_element_on_one_line_whatever_line_break_its_texts_hold(
    #[values("\n", "\r", "\r\n", "\u{85}", "\u{2028}", "\u{2029}")] line_break: &str,
) {
    let text = format!("x{line_break}y");
    let snapshot = snapshot(
        &text,
        vec![
            element("a", &text, &text, Some(&text)),
            element("b", "t", &text, None),
        ],
        &[],
    );

    let outline = outline_of(&snapshot);

    assert_eq!(outline.lines().count(), 4, "{outline:?}");
    let inner = outline.trim_end_matches('\n');
    for other in LINE_BREAKS {
        assert_eq!(
            inner.matches(other).count(),
            3 * usize::from(other == "\n"),
            "{other:?}"
        );
    }
    let escaped = one_line(line_break);
    assert_eq!(outline.matches(&escaped).count(), 5, "{outline:?}");
}

#[test]
fn escapes_terminal_and_other_control_characters_in_the_title() {
    let snapshot = snapshot("\u{1b}[31mred\u{0}\u{7f}\u{9b}", Vec::new(), &[]);

    assert_eq!(
        outline_of(&snapshot),
        "Board: b · \\u{1b}[31mred\\u{0}\\u{7f}\\u{9b}\n\n"
    );
}

#[rstest]
#[case::right_to_left_override('\u{202e}', "\\u{202e}")]
#[case::left_to_right_isolate('\u{2066}', "\\u{2066}")]
#[case::arabic_letter_mark('\u{61c}', "\\u{61c}")]
#[case::zero_width_space('\u{200b}', "\\u{200b}")]
#[case::byte_order_mark('\u{feff}', "\\u{feff}")]
#[case::soft_hyphen('\u{ad}', "\\u{ad}")]
#[case::nominal_digit_shapes('\u{206f}', "\\u{206f}")]
#[case::interlinear_annotation('\u{fff9}', "\\u{fff9}")]
#[case::tag_language('\u{e0001}', "\\u{e0001}")]
#[case::tag_letter('\u{e0049}', "\\u{e0049}")]
#[case::cancel_tag('\u{e007f}', "\\u{e007f}")]
fn escapes_invisible_and_reordering_characters(#[case] character: char, #[case] expected: &str) {
    let name = format!("a{character}b");
    let snapshot = snapshot("Board", vec![element("a", "t", &name, None)], &[]);

    assert!(
        outline_of(&snapshot).ends_with(&format!("a  t  a{expected}b\n")),
        "{:?}",
        outline_of(&snapshot)
    );
}

#[test]
fn escapes_the_first_and_last_character_of_every_range() {
    for range in &FORMAT_AND_SEPARATORS {
        for character in [*range.start(), *range.end()] {
            assert_eq!(
                one_line(&character.to_string()),
                character.escape_unicode().to_string()
            );
        }
    }
}

#[test]
fn hides_a_tag_sequence_from_no_reader() {
    let smuggled: String = "\u{e0001}\u{e0049}\u{e0067}\u{e006e}\u{e006f}\u{e0072}\u{e0065}".into();

    assert_eq!(
        one_line(&format!("ok{smuggled}")),
        "ok\\u{e0001}\\u{e0049}\\u{e0067}\\u{e006e}\\u{e006f}\\u{e0072}\\u{e0065}"
    );
}

#[test]
fn tells_a_written_backslash_n_from_an_escaped_line_feed() {
    assert_eq!(one_line("a\\nb"), "a\\\\nb");
    assert_eq!(one_line("a\nb"), "a\\nb");
    assert_ne!(one_line("a\\nb"), one_line("a\nb"));
}

#[rstest]
#[case::precomposed_umlaut("Zürich")]
#[case::decomposed_umlaut("Zu\u{308}rich")]
#[case::cjk("支付服务")]
#[case::emoji_with_variation_selector("☎\u{fe0f}")]
#[case::arabic("خدمة الدفع")]
#[case::space("Payment Service")]
fn keeps_ordinary_text_as_it_is(#[case] text: &str) {
    assert_eq!(one_line(text), text);
}

#[test]
fn a_long_name_does_not_pad_the_other_lines() {
    let long = "n".repeat(10_000);
    let mut elements = vec![element("long", "t", &long, Some("s"))];
    elements.extend((0..100).map(|index| element(&format!("e{index}"), "t", "short", Some("s"))));
    let snapshot = snapshot("Board", elements, &[]);

    let outline = outline_of(&snapshot);

    let bound = 3 * (MAX_COLUMN_WIDTH + 2) + "s".len();
    let short_lines: Vec<&str> = outline
        .lines()
        .filter(|line| line.starts_with('e'))
        .collect();
    assert_eq!(short_lines.len(), 100);
    for line in short_lines {
        assert!(
            line.chars().count() <= bound,
            "{} characters",
            line.chars().count()
        );
    }
}

/// A label made often of the characters that break, hide, or reorder text.
fn arbitrary_label() -> impl Strategy<Value = String> {
    let dangerous = prop::sample::select(vec![
        '\n',
        '\r',
        '\t',
        '\u{85}',
        '\u{2028}',
        '\u{2029}',
        '\u{202e}',
        '\u{2066}',
        '\u{feff}',
        '\u{e0041}',
        '\\',
        'a',
        '→',
        ' ',
    ]);
    prop_oneof![
        any::<String>(),
        prop::collection::vec(dangerous, 0..8)
            .prop_map(|characters| characters.into_iter().collect()),
    ]
}

/// Relations of element `index` to the elements `e0` to `e7`, of which only
/// the ones a test generates lie on the board.
fn arbitrary_relations(index: usize) -> impl Strategy<Value = Relations> {
    let kind = prop::sample::select(vec![
        RelationKind::Concerns,
        RelationKind::DependsOn,
        RelationKind::Describes,
        RelationKind::SeeAlso,
        RelationKind::Uses,
    ]);
    let target = (0..8_usize)
        .prop_filter("a relation to itself", move |target| *target != index)
        .prop_filter_map("invalid ID", |target| format!("e{target}").parse().ok());
    prop::collection::btree_map(
        (kind, target),
        proptest::option::of(arbitrary_label()),
        0..3,
    )
}

/// An element with arbitrary type, name, summary, and relations, if the
/// element file can hold them.
fn arbitrary_element(index: usize) -> impl Strategy<Value = Option<Element>> {
    (
        any::<String>(),
        any::<String>(),
        proptest::option::of(any::<String>()),
        arbitrary_relations(index),
    )
        .prop_map(move |(type_ref, name, summary, relations)| {
            let type_ref = TypeRef::try_from(type_ref).ok()?;
            let id = format!("e{index}").parse().ok()?;
            Element::new(
                id,
                ElementFields {
                    type_ref,
                    name,
                    summary,
                    body: String::new(),
                    relations,
                    anchor: None,
                    verified: None,
                },
            )
            .ok()
        })
}

proptest! {
    #[test]
    fn writes_one_line_per_element_and_relation_without_any_escaped_character(
        title in any::<String>(),
        elements in (0..6_usize).prop_flat_map(|count| {
            (0..count).map(arbitrary_element).collect::<Vec<_>>()
        }),
    ) {
        let elements: Vec<Element> = elements.into_iter().flatten().collect();
        let count = elements.len();
        let placed: Vec<&ElementId> = elements.iter().map(Element::id).collect();
        let relations: usize = elements
            .iter()
            .map(|element| {
                element
                    .relations()
                    .keys()
                    .filter(|(_, to)| placed.contains(&to))
                    .count()
            })
            .sum();
        let expected = 2 + count + if relations > 0 { 1 + relations } else { 0 };
        let snapshot = snapshot(&title, elements, &[]);

        let outline = outline_of(&snapshot);

        prop_assert_eq!(outline.lines().count(), expected);
        prop_assert_eq!(outline.matches('\n').count(), expected);
        let inner = outline.replace('\n', "");
        prop_assert!(
            !inner.chars().any(hides_or_breaks_text),
            "{:?}",
            outline
        );
    }
}

#[test]
fn lists_elements_in_the_given_order_and_marks_missing_ones() {
    let snapshot = snapshot(
        "Board",
        vec![
            element("a", "t", "A", None),
            element("b", "t", "B", Some("second")),
            element("c", "t", "C", None),
        ],
        &[],
    );
    let ids: Vec<ElementId> = ["c", "a", "gone", "b"]
        .iter()
        .map(|id| id.parse().unwrap())
        .collect();

    assert_eq!(
        element_list(&snapshot, &ids),
        "c     t  C\n\
         a     t  A\n\
         gone  ⚠ missing\n\
         b     t  B  second\n"
    );
}

#[test]
fn lists_an_element_given_twice_twice() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", None)], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(element_list(&snapshot, [&id, &id]), "a  t  A\na  t  A\n");
}

#[test]
fn lists_nothing_for_no_ids() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", None)], &[]);

    assert_eq!(element_list(&snapshot, []), "");
}

#[test]
fn escapes_element_text_in_the_list() {
    let snapshot = snapshot("Board", vec![element("a", "t", "x\ny\u{202e}z", None)], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(element_list(&snapshot, [&id]), "a  t  x\\ny\\u{202e}z\n");
}

fn sized(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width: NonZeroU32::new(width).unwrap(),
        height: NonZeroU32::new(height).unwrap(),
    }
}

/// `snapshot` with the placement of `id` on board `b` at `rect`.
fn with_rect(mut snapshot: Snapshot, id: &str, rect: Rect) -> Snapshot {
    snapshot
        .boards
        .get_mut(&"b".parse().unwrap())
        .unwrap()
        .placements
        .insert(id.parse().unwrap(), rect);
    snapshot
}

fn outline_with_geometry(snapshot: &Snapshot) -> String {
    outline(snapshot, &"b".parse().unwrap(), Geometry::Shown).unwrap()
}

#[test]
fn shows_position_and_size_after_the_id_on_request() {
    let snapshot = snapshot(
        "T",
        vec![
            element("a", "t", "A", Some("first")),
            element("b", "t", "B", None),
        ],
        &["c"],
    );
    let snapshot = with_rect(snapshot, "a", sized(-40, 120, 180, 90));
    let snapshot = with_rect(snapshot, "b", sized(400, -7, 300, 50));
    let snapshot = with_rect(snapshot, "c", sized(10, 20, 1, 1));

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\
         \n\
         a  @-40,120 180x90  t  A  first\n\
         b  @400,-7 300x50   t  B\n\
         c  @10,20 1x1       ⚠ missing\n"
    );
}

#[test]
fn shows_the_extreme_positions_and_sizes_a_board_file_holds() {
    let snapshot = snapshot("T", vec![element("a", "t", "A", None)], &[]);
    let snapshot = with_rect(snapshot, "a", sized(i32::MIN, i32::MAX, u32::MAX, u32::MAX));

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\na  @-2147483648,2147483647 4294967295x4294967295  t  A\n"
    );
}

#[test]
fn shows_the_geometry_before_any_text_from_the_repository() {
    let snapshot = snapshot(
        "T",
        vec![element("a", "t", "@1,2 3x4\nZ\u{202e}", None)],
        &[],
    );
    let snapshot = with_rect(snapshot, "a", sized(5, 6, 7, 8));

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\na  @5,6 7x8  t  @1,2 3x4\\nZ\\u{202e}\n"
    );
}

/// `element` with the given relations.
fn related(element: Element, relations: &[(RelationKind, &str, Option<&str>)]) -> Element {
    let id = element.id().clone();
    let relations = relations
        .iter()
        .map(|(kind, to, label)| ((*kind, to.parse().unwrap()), label.map(str::to_owned)))
        .collect();
    Element::new(
        id,
        ElementFields {
            relations,
            ..element.into_fields()
        },
    )
    .unwrap()
}

/// The lines of an outline after the blank line that ends its elements.
fn relation_lines(outline: &str) -> Vec<&str> {
    outline
        .split("\n\n")
        .nth(2)
        .map(|part| part.lines().collect())
        .unwrap_or_default()
}

#[test]
fn renders_relations_after_the_elements_by_source_then_kind_and_target() {
    let snapshot = snapshot(
        "Container Level",
        vec![
            related(
                element("checkout", "c4/container", "Checkout", None),
                &[
                    (
                        RelationKind::Uses,
                        "payment-service",
                        Some("REST, synchronous"),
                    ),
                    (RelationKind::Uses, "orders-db", Some("writes orders")),
                ],
            ),
            element("orders-db", "c4/datastore", "Orders DB", None),
            related(
                element("payment-service", "c4/container", "Payment Service", None),
                &[(RelationKind::DependsOn, "orders-db", None)],
            ),
        ],
        &[],
    );

    assert_eq!(
        outline_of(&snapshot),
        "Board: b · Container Level\n\
         \n\
         checkout         c4/container  Checkout\n\
         orders-db        c4/datastore  Orders DB\n\
         payment-service  c4/container  Payment Service\n\
         \n\
         checkout → orders-db         uses  writes orders\n\
         checkout → payment-service   uses  REST, synchronous\n\
         payment-service → orders-db  depends_on\n"
    );
}

#[test]
fn shows_only_relations_whose_ends_both_lie_on_the_board() {
    let mut snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[
                    (RelationKind::Uses, "b", None),
                    (RelationKind::Uses, "elsewhere", None),
                    (RelationKind::Uses, "ghost", None),
                    (RelationKind::Uses, "nowhere", None),
                ],
            ),
            element("b", "t", "B", None),
        ],
        &["ghost"],
    );
    let elsewhere = element("elsewhere", "t", "Elsewhere", None);
    let off_board = related(
        element("off-board", "t", "Off board", None),
        &[(RelationKind::Uses, "a", None)],
    );
    snapshot.elements.insert(elsewhere.id().clone(), elsewhere);
    snapshot.elements.insert(off_board.id().clone(), off_board);

    let outline = outline_of(&snapshot);

    assert_eq!(
        relation_lines(&outline),
        ["a → b      uses", "a → ghost  uses"]
    );
}

#[test]
fn renders_a_board_whose_relations_all_point_elsewhere_as_without_relations() {
    let with_relations = snapshot(
        "Board",
        vec![related(
            element("a", "t", "A", Some("first")),
            &[(RelationKind::SeeAlso, "elsewhere", Some("x"))],
        )],
        &[],
    );
    let without = snapshot("Board", vec![element("a", "t", "A", Some("first"))], &[]);

    assert_eq!(outline_of(&with_relations), outline_of(&without));
    assert_eq!(outline_of(&without), "Board: b · Board\n\na  t  A  first\n");
}

#[test]
fn leaves_out_an_empty_label_like_a_missing_one() {
    let snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[(RelationKind::Uses, "b", Some(""))],
            ),
            element("b", "t", "B", None),
        ],
        &[],
    );

    assert_eq!(relation_lines(&outline_of(&snapshot)), ["a → b  uses"]);
}

#[test]
fn shows_the_same_relation_lines_with_and_without_geometry() {
    let snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[(RelationKind::Uses, "b", Some("REST"))],
            ),
            element("b", "t", "B", None),
        ],
        &[],
    );

    assert_eq!(
        relation_lines(&outline_with_geometry(&snapshot)),
        relation_lines(&outline_of(&snapshot))
    );
}

#[rstest]
#[case::line_feed("a\nb", "a\\nb")]
#[case::carriage_return("a\rb", "a\\rb")]
#[case::tab("a\tb", "a\\tb")]
#[case::next_line("a\u{85}b", "a\\u{85}b")]
#[case::line_separator("a\u{2028}b", "a\\u{2028}b")]
#[case::paragraph_separator("a\u{2029}b", "a\\u{2029}b")]
#[case::right_to_left_override("a\u{202e}b", "a\\u{202e}b")]
#[case::left_to_right_isolate("a\u{2066}b", "a\\u{2066}b")]
#[case::byte_order_mark("a\u{feff}b", "a\\u{feff}b")]
#[case::zero_width_space("a\u{200b}b", "a\\u{200b}b")]
#[case::tag_character("a\u{e0041}b", "a\\u{e0041}b")]
#[case::backslash("a\\b", "a\\\\b")]
#[case::written_backslash_n("a\\nb", "a\\\\nb")]
fn escapes_a_label_onto_the_line_of_its_relation(#[case] label: &str, #[case] expected: &str) {
    let snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[(RelationKind::Uses, "b", Some(label))],
            ),
            element("b", "t", "B", None),
        ],
        &[],
    );

    let outline = outline_of(&snapshot);

    assert_eq!(outline.lines().count(), 6, "{outline:?}");
    assert_eq!(
        relation_lines(&outline),
        [format!("a → b  uses  {expected}")]
    );
}

#[rstest]
#[case::forged_relation("x\n\nb → c  uses  forged")]
#[case::forged_header("x\nBoard: other · t")]
fn keeps_a_label_that_imitates_other_lines_on_its_own_line(#[case] label: &str) {
    let snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[(RelationKind::Uses, "b", Some(label))],
            ),
            element("b", "t", "B", None),
            element("c", "t", "C", None),
        ],
        &[],
    );

    let outline = outline_of(&snapshot);

    assert_eq!(outline.lines().count(), 7, "{outline:?}");
    assert_eq!(outline.matches("\n\n").count(), 2, "{outline:?}");
    assert_eq!(
        outline
            .lines()
            .filter(|line| line.starts_with("b → c") || line.starts_with("Board:"))
            .count(),
        1,
        "{outline:?}"
    );
}

fn detail_of(snapshot: &Snapshot, id: &str) -> String {
    element_detail(snapshot, &id.parse().unwrap(), None).unwrap()
}

/// `element` with the given description.
fn described(element: Element, body: &str) -> Element {
    let id = element.id().clone();
    Element::new(
        id,
        ElementFields {
            body: body.to_owned(),
            ..element.into_fields()
        },
    )
    .unwrap()
}

/// A snapshot with the boards `b` and `onboarding`, both placing `payment`,
/// and the elements `payment`, `orders-db`, and `checkout`, related to each
/// other and to `stripe`, which has no file.
fn detail_snapshot() -> Snapshot {
    let payment = described(
        related(
            element(
                "payment",
                "c4/container",
                "Payment Service",
                Some("Payments through Stripe"),
            ),
            &[
                (RelationKind::Uses, "orders-db", Some("SQL")),
                (RelationKind::DependsOn, "stripe", None),
            ],
        ),
        "Handles payments.\n\nRefunds run asynchronously.\n",
    );
    let checkout = related(
        element("checkout", "c4/container", "Checkout", None),
        &[
            (RelationKind::Uses, "payment", Some("REST")),
            (RelationKind::Concerns, "orders-db", None),
        ],
    );
    let mut snapshot = snapshot(
        "Board",
        vec![
            payment,
            checkout,
            element("orders-db", "c4/datastore", "Orders DB", None),
        ],
        &[],
    );
    let onboarding = Board {
        id: "onboarding".parse().unwrap(),
        title: "Onboarding".to_owned(),
        placements: BTreeMap::from([("payment".parse().unwrap(), rect())]),
    };
    snapshot.boards.insert(onboarding.id.clone(), onboarding);
    snapshot
}

#[test]
fn renders_the_detail_of_an_element() {
    assert_eq!(
        detail_of(&detail_snapshot(), "payment"),
        "Element: payment · Payment Service\n\
         Type: c4/container\n\
         Anchor: none\n\
         Verified: none\n\
         Summary: Payments through Stripe\n\
         Boards: b, onboarding\n\
         \n\
         Outgoing relations:\n\
         payment → stripe ⚠ missing  depends_on\n\
         payment → orders-db         uses  SQL\n\
         \n\
         Incoming relations:\n\
         checkout → payment  uses  REST\n\
         \n\
         Description:\n\
         Handles payments.\n\
         \n\
         Refunds run asynchronously.\n"
    );
}

/// `element` with the given anchor and stamp.
fn anchored(element: Element, anchor: &str, stamp: Option<(&str, Verifier)>) -> Element {
    let id = element.id().clone();
    Element::new(
        id,
        ElementFields {
            anchor: Some(Anchor::try_from(anchor.to_owned()).unwrap()),
            verified: stamp.map(|(commit, by)| Verification {
                commit: CommitId::try_from(commit.to_owned()).unwrap(),
                by,
            }),
            ..element.into_fields()
        },
    )
    .unwrap()
}

#[rstest]
#[case::sha1_by_person("0123456789abcdef0123456789abcdef01234567", Verifier::Person, "person")]
#[case::sha256_by_ai(&"fedcba9876543210".repeat(4), Verifier::Ai, "ai")]
fn renders_the_anchor_and_the_stamp_after_the_type(
    #[case] commit: &str,
    #[case] by: Verifier,
    #[case] name: &str,
) {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let element = anchored(
        element("x", "t", "X", Some("S")),
        "crates/payment service/Überweisung.md",
        Some((commit, by)),
    );
    snapshot.elements.insert(element.id().clone(), element);

    assert!(
        detail_of(&snapshot, "x").starts_with(&format!(
            "Element: x · X\n\
             Type: t\n\
             Anchor: crates/payment service/Überweisung.md\n\
             Verified: {commit} by {name}\n\
             Summary: S\n\
             Boards: none\n"
        )),
        "{}",
        detail_of(&snapshot, "x")
    );
}

#[test]
fn renders_an_anchor_without_a_stamp() {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let element = anchored(element("x", "t", "X", None), "docs", None);
    snapshot.elements.insert(element.id().clone(), element);

    assert!(
        detail_of(&snapshot, "x").contains("\nAnchor: docs\nVerified: none\n"),
        "{}",
        detail_of(&snapshot, "x")
    );
}

#[test]
fn renders_none_for_missing_boards_relations_and_leaves_out_a_missing_summary() {
    let mut snapshot = detail_snapshot();
    let loose = element("loose", "t", "Loose", None);
    snapshot.elements.insert(loose.id().clone(), loose);

    assert_eq!(
        detail_of(&snapshot, "loose"),
        "Element: loose · Loose\n\
         Type: t\n\
         Anchor: none\n\
         Verified: none\n\
         Boards: none\n\
         \n\
         Outgoing relations:\n\
         none\n\
         \n\
         Incoming relations:\n\
         none\n\
         \n\
         Description:\n"
    );
}

#[test]
fn lists_incoming_relations_by_source_then_kind() {
    let detail = detail_of(&detail_snapshot(), "orders-db");

    assert!(
        detail.contains(
            "Incoming relations:\n\
             checkout → orders-db  concerns\n\
             payment → orders-db   uses  SQL\n\n"
        ),
        "{detail}"
    );
}

#[rstest]
#[case::unknown("nowhere")]
#[case::placed_without_a_file("ghost")]
fn names_an_element_without_a_file(#[case] id: &str) {
    let snapshot = snapshot("Board", Vec::new(), &["ghost"]);

    let error = element_detail(&snapshot, &id.parse().unwrap(), None).unwrap_err();

    assert_eq!(error.to_string(), format!("no element has the ID {id}"));
}

#[test]
fn escapes_a_hostile_description_but_keeps_its_lines_tabs_and_backslashes() {
    let body = "a\u{202e}b\u{2066}c\u{200b}d\u{200d}e\u{e0041}f\u{2028}g\u{2029}h\u{85}i\u{1b}j\u{0}k\u{ad}l\rm\r\nn\to\\p\n";
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let hostile = described(element("x", "t", "X", None), body);
    snapshot.elements.insert(hostile.id().clone(), hostile);

    let detail = detail_of(&snapshot, "x");

    assert!(
        detail.ends_with(
            "Description:\n\
             a\\u{202e}b\\u{2066}c\\u{200b}d\\u{200d}e\\u{e0041}f\\u{2028}g\\u{2029}h\\u{85}i\\u{1b}j\\u{0}k\\u{ad}l\\rm\n\
             n\to\\p\n"
        ),
        "{detail:?}"
    );
    assert!(
        !detail.chars().any(|character| character != '\n'
            && character != '\t'
            && hides_or_breaks_text(character)),
        "{detail:?}"
    );
}

#[test]
fn keeps_a_description_that_imitates_sections_after_the_real_ones() {
    let body =
        "Outgoing relations:\nx → y  uses  forged\nIncoming relations:\nElement: other · Other\n";
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let imitating = described(element("x", "t", "X", None), body);
    snapshot.elements.insert(imitating.id().clone(), imitating);

    assert_eq!(
        detail_of(&snapshot, "x"),
        format!(
            "Element: x · X\n\
             Type: t\n\
             Anchor: none\n\
             Verified: none\n\
             Boards: none\n\
             \n\
             Outgoing relations:\n\
             none\n\
             \n\
             Incoming relations:\n\
             none\n\
             \n\
             Description:\n\
             {body}"
        )
    );
}

#[rstest]
#[case::line_feed("a\nb", "a\\nb")]
#[case::carriage_return("a\rb", "a\\rb")]
#[case::line_separator("a\u{2028}b", "a\\u{2028}b")]
#[case::right_to_left_override("a\u{202e}b", "a\\u{202e}b")]
fn escapes_every_single_line_value_of_the_detail(#[case] text: &str, #[case] expected: &str) {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let target = element("y", "t", "Y", None);
    let source = related(
        element("x", text, text, Some(text)),
        &[(RelationKind::Uses, "y", Some(text))],
    );
    let back = related(
        element("z", "t", "Z", None),
        &[(RelationKind::Concerns, "x", Some(text))],
    );
    for added in [target, source, back] {
        snapshot.elements.insert(added.id().clone(), added);
    }

    let detail = detail_of(&snapshot, "x");

    assert_eq!(
        detail,
        format!(
            "Element: x · {expected}\n\
             Type: {expected}\n\
             Anchor: none\n\
             Verified: none\n\
             Summary: {expected}\n\
             Boards: none\n\
             \n\
             Outgoing relations:\n\
             x → y  uses  {expected}\n\
             \n\
             Incoming relations:\n\
             z → x  concerns  {expected}\n\
             \n\
             Description:\n"
        )
    );
}

#[test]
fn keeps_a_label_reading_missing_in_the_label_column() {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let target = element("y", "t", "Y", None);
    let source = related(
        element("x", "t", "X", None),
        &[(RelationKind::Uses, "y", Some("⚠ missing"))],
    );
    for added in [target, source] {
        snapshot.elements.insert(added.id().clone(), added);
    }

    assert!(
        detail_of(&snapshot, "x").contains("Outgoing relations:\nx → y  uses  ⚠ missing\n"),
        "{}",
        detail_of(&snapshot, "x")
    );
}

const STAMP: &str = "0123456789abcdef0123456789abcdef01234567";

/// The snapshot with only the element `x`, anchored on `crates/x` and
/// stamped with `STAMP`.
fn stamped_snapshot() -> Snapshot {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let element = anchored(
        element("x", "t", "X", None),
        "crates/x",
        Some((STAMP, Verifier::Person)),
    );
    snapshot.elements.insert(element.id().clone(), element);
    snapshot
}

/// Facts in which the stamp `STAMP` is known and the index changed
/// `since` it.
fn facts(known: &[&[u8]], since: &[&[u8]]) -> GitFacts {
    let paths = |values: &[&[u8]]| values.iter().map(|value| value.to_vec()).collect();
    GitFacts {
        known: paths(known),
        uncommitted: BTreeSet::new(),
        since: BTreeMap::from([(
            CommitId::try_from(STAMP.to_owned()).unwrap(),
            Since::Changed(paths(since)),
        )]),
    }
}

fn reviewed_detail(snapshot: &Snapshot, facts: &GitFacts) -> String {
    let review = review(snapshot, facts);
    element_detail(snapshot, &"x".parse().unwrap(), Some(&review)).unwrap()
}

#[test]
fn renders_review_none_for_an_element_without_flags() {
    let snapshot = stamped_snapshot();

    let detail = reviewed_detail(&snapshot, &facts(&[b"crates/x/lib.rs"], &[]));

    assert!(
        detail.contains(&format!(
            "\nVerified: {STAMP} by person\nReview: none\nBoards: none\n"
        )),
        "{detail}"
    );
}

#[test]
fn renders_each_flag_on_its_own_line_after_the_highest_priority() {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let element = anchored(element("x", "t", "X", Some("S")), "crates/gone", None);
    snapshot.elements.insert(element.id().clone(), element);

    let detail = reviewed_detail(&snapshot, &facts(&[], &[]));

    assert!(
        detail.contains(
            "\nVerified: none\n\
             Review: task\n  \
             never verified\n  \
             no file that Git knows lies under the anchor\n\
             Summary: S\n"
        ),
        "{detail}"
    );
}

#[test]
fn names_at_most_ten_changed_files_escaped_and_counts_the_rest() {
    let snapshot = stamped_snapshot();
    let mut since: Vec<Vec<u8>> = (0..11)
        .map(|index| format!("crates/x/{index:02}.rs").into_bytes())
        .collect();
    since.extend([
        b"crates/x/a\nb".to_vec(),
        b"crates/x/\xe2\x80\xaeevil".to_vec(),
        b"crates/x/\xff".to_vec(),
    ]);
    let since: Vec<&[u8]> = since.iter().map(Vec::as_slice).collect();

    let detail = reviewed_detail(&snapshot, &facts(&[b"crates/x/00.rs"], &since));

    let line = detail
        .lines()
        .find(|line| line.starts_with("  changed since the stamp: "))
        .unwrap();
    assert_eq!(
        line,
        "  changed since the stamp: crates/x/00.rs, crates/x/01.rs, crates/x/02.rs, \
         crates/x/03.rs, crates/x/04.rs, crates/x/05.rs, crates/x/06.rs, crates/x/07.rs, \
         crates/x/08.rs, crates/x/09.rs and 4 more"
    );
}

#[test]
fn escapes_hostile_file_names_in_a_review() {
    let snapshot = stamped_snapshot();
    let since: [&[u8]; 3] = [
        b"crates/x/a\nb",
        b"crates/x/\xe2\x80\xaeevil",
        b"crates/x/\xff",
    ];

    let detail = reviewed_detail(&snapshot, &facts(&[b"crates/x/a\nb"], &since));

    assert!(
        detail.contains(
            "  changed since the stamp: crates/x/a\\nb, crates/x/\\u{202e}evil, crates/x/\u{fffd}\n"
        ),
        "{detail}"
    );
}
