use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use abyme_model::{
    Anchor, CommitId, ElementFields, FORMAT_AND_SEPARATORS, Mode, Placement, Rect, Relations,
    TypeRef, Verification, Verifier,
};
use proptest::prelude::*;
use rstest::rstest;

use super::*;
use crate::review::{GitFacts, RepositoryFacts, Since, review};

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
            statement: None,
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
        owner: None,
        summary: None,
        zone: None,
        placements: placed
            .map(|id| {
                (
                    id,
                    Placement {
                        rect: rect(),
                        pinned: false,
                    },
                )
            })
            .collect(),
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
    outline(snapshot, &"b".parse().unwrap(), Geometry::Hidden, None).unwrap()
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
        None,
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
                    statement: None,
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
        element_list(&snapshot, &ids, None),
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

    assert_eq!(
        element_list(&snapshot, [&id, &id], None),
        "a  t  A\na  t  A\n"
    );
}

#[test]
fn lists_nothing_for_no_ids() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", None)], &[]);

    assert_eq!(element_list(&snapshot, [], None), "");
}

#[test]
fn escapes_element_text_in_the_list() {
    let snapshot = snapshot("Board", vec![element("a", "t", "x\ny\u{202e}z", None)], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(
        element_list(&snapshot, [&id], None),
        "a  t  x\\ny\\u{202e}z\n"
    );
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
        .insert(
            id.parse().unwrap(),
            Placement {
                rect,
                pinned: false,
            },
        );
    snapshot
}

fn outline_with_geometry(snapshot: &Snapshot) -> String {
    outline(snapshot, &"b".parse().unwrap(), Geometry::Shown, None).unwrap()
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

#[test]
fn shows_a_pin_after_the_size_only_with_the_geometry() {
    let snapshot = snapshot("T", vec![element("a", "t", "n", None)], &[]);
    let mut snapshot = with_rect(snapshot, "a", sized(5, 6, 7, 8));
    let unpinned_without_geometry = outline_of(&snapshot);
    snapshot
        .boards
        .get_mut(&"b".parse().unwrap())
        .unwrap()
        .placements
        .get_mut(&"a".parse().unwrap())
        .unwrap()
        .pinned = true;

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\na  @5,6 7x8 pinned  t  n\n"
    );
    assert_eq!(outline_of(&snapshot), unpinned_without_geometry);
}

#[test]
fn a_name_that_says_pinned_marks_no_position() {
    let snapshot = snapshot("T", vec![element("a", "t", "pinned", None)], &[]);
    let snapshot = with_rect(snapshot, "a", sized(5, 6, 7, 8));

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\na  @5,6 7x8  t  pinned\n"
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
        owner: None,
        summary: None,
        zone: None,
        placements: BTreeMap::from([(
            "payment".parse().unwrap(),
            Placement {
                rect: rect(),
                pinned: false,
            },
        )]),
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
         Statement kind: none\n\
         Anchor: none\n\
         Verified: none\n\
         Summary: Payments through Stripe\n\
         Boards: b, onboarding\n\
         Inner board: none\n\
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
             Statement kind: none\n\
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
         Statement kind: none\n\
         Anchor: none\n\
         Verified: none\n\
         Boards: none\n\
         Inner board: none\n\
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
             Statement kind: none\n\
             Anchor: none\n\
             Verified: none\n\
             Boards: none\n\
             Inner board: none\n\
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
             Statement kind: none\n\
             Anchor: none\n\
             Verified: none\n\
             Summary: {expected}\n\
             Boards: none\n\
             Inner board: none\n\
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
        project: RepositoryFacts {
            known: paths(known),
            uncommitted: BTreeSet::new(),
            since: BTreeMap::from([(
                CommitId::try_from(STAMP.to_owned()).unwrap(),
                Since::Changed(paths(since)),
            )]),
        },
        nested: BTreeMap::new(),
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

/// A snapshot whose board `b` places `a`, anchored on `crates/a` and
/// stamped, `b`, stamped and using `a`, and `c`, without flags, with the
/// facts in which `crates/a/lib.rs` changed since the stamp; and its review.
fn marked() -> (Snapshot, Review) {
    let a = anchored(
        element("a", "t", "A", None),
        "crates/a",
        Some((STAMP, Verifier::Ai)),
    );
    let b = anchored(
        related(
            element("b", "t", "B", Some("S")),
            &[(RelationKind::Uses, "a", None)],
        ),
        "crates/b",
        Some((STAMP, Verifier::Ai)),
    );
    let snapshot = snapshot("Board", vec![a, b, element("c", "t", "C", None)], &[]);
    let review = review(
        &snapshot,
        &facts(
            &[b"crates/a/lib.rs", b"crates/b/lib.rs"],
            &[b"crates/a/lib.rs"],
        ),
    );
    (snapshot, review)
}

#[test]
fn marks_the_highest_priority_last_on_each_element_line() {
    let (snapshot, review) = marked();

    let outline = outline(
        &snapshot,
        &"b".parse().unwrap(),
        Geometry::Hidden,
        Some(&review),
    )
    .unwrap();

    assert_eq!(
        outline,
        "Board: b · Board\n\
         \n\
         a  t  A     ⚠ task\n\
         b  t  B  S  ⚠ hint\n\
         c  t  C\n\
         \n\
         b → a  uses\n"
    );
}

#[test]
fn marks_search_lines_like_board_lines() {
    let (snapshot, review) = marked();
    let ids: Vec<ElementId> = ["c", "a"].iter().map(|id| id.parse().unwrap()).collect();

    assert_eq!(
        element_list(&snapshot, &ids, Some(&review)),
        "c  t  C\na  t  A    ⚠ task\n"
    );
}

#[test]
fn marks_nothing_without_a_review() {
    let (snapshot, _) = marked();

    let outline = outline(&snapshot, &"b".parse().unwrap(), Geometry::Hidden, None).unwrap();

    assert!(!outline.contains('⚠'), "{outline}");
}

#[test]
fn keeps_the_marker_last_when_lines_show_coordinates() {
    let (snapshot, review) = marked();

    let outline = outline(
        &snapshot,
        &"b".parse().unwrap(),
        Geometry::Shown,
        Some(&review),
    )
    .unwrap();

    let line = outline.lines().find(|line| line.starts_with("a ")).unwrap();
    assert_eq!(line, "a  @0,0 1x1  t  A     ⚠ task");
}

#[test]
fn reports_no_open_review_flags() {
    let snapshot = snapshot("Board", vec![element("c", "t", "C", None)], &[]);
    let review = review(&snapshot, &facts(&[], &[]));

    assert_eq!(status_report(&snapshot, &review), "No open review flags.");
}

#[test]
fn reports_tasks_before_hints_after_a_count() {
    let (snapshot, review) = marked();

    assert_eq!(
        status_report(&snapshot, &review),
        "Elements to review: 1 with a task, 1 with only a hint.\n\
         a  t  A     ⚠ task\n\
         b  t  B  S  ⚠ hint\n"
    );
}

#[test]
fn reports_hostile_names_on_one_line_each() {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let hostile = anchored(
        element("x", "t", "a\nb\tc", Some("c\rd\u{202e}")),
        "crates/x",
        None,
    );
    snapshot.elements.insert(hostile.id().clone(), hostile);
    let review = review(&snapshot, &facts(&[b"crates/x/lib.rs"], &[]));

    let report = status_report(&snapshot, &review);

    assert_eq!(report.lines().count(), 2, "{report:?}");
    assert!(
        report.ends_with("x  t  a\\nb\\tc  c\\rd\\u{202e}  ⚠ task\n"),
        "{report:?}"
    );
}

#[test]
fn reports_at_most_fifty_elements_and_counts_all() {
    let tasks = (0..55).map(|number| {
        anchored(
            element(&format!("e{number:02}"), "t", "E", None),
            &format!("crates/e{number:02}"),
            None,
        )
    });
    let hints = (0..10).map(|number| {
        anchored(
            related(
                element(&format!("h{number}"), "t", "H", None),
                &[(RelationKind::Uses, "e00", None)],
            ),
            &format!("docs/h{number}"),
            Some((STAMP, Verifier::Ai)),
        )
    });
    let snapshot = snapshot("Board", tasks.chain(hints).collect(), &[]);
    let known: Vec<Vec<u8>> = (0..55)
        .map(|number| format!("crates/e{number:02}/lib.rs").into_bytes())
        .chain((0..10).map(|number| format!("docs/h{number}/a.md").into_bytes()))
        .collect();
    let known: Vec<&[u8]> = known.iter().map(Vec::as_slice).collect();
    let review = review(&snapshot, &facts(&known, &[b"crates/e00/lib.rs"]));

    let report = status_report(&snapshot, &review);

    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(
        lines.first(),
        Some(&"Elements to review: 55 with a task, 10 with only a hint.")
    );
    assert_eq!(lines.len(), 1 + 50 + 1, "{report}");
    assert!(
        lines.get(1).is_some_and(|line| line.starts_with("e00 ")),
        "{report}"
    );
    assert!(
        lines.get(50).is_some_and(|line| line.starts_with("e49 ")),
        "{report}"
    );
    assert_eq!(lines.last(), Some(&"… and 15 more"));
}

/// The board `id`, titled `id` too, the inside of `owner`, that places the
/// elements `placed`.
fn hierarchy_board(id: &str, owner: Option<&str>, placed: &[&str]) -> Board {
    Board {
        id: id.parse().unwrap(),
        title: id.to_owned(),
        owner: owner.map(|owner| owner.parse().unwrap()),
        summary: None,
        zone: None,
        placements: placed
            .iter()
            .map(|element| {
                (
                    element.parse().unwrap(),
                    Placement {
                        rect: rect(),
                        pinned: false,
                    },
                )
            })
            .collect(),
    }
}

/// A snapshot of `boards` and of a file for each element in `elements`,
/// typed `t` and named after its ID in capitals.
fn hierarchy(boards: Vec<Board>, elements: &[&str]) -> Snapshot {
    Snapshot {
        elements: elements
            .iter()
            .map(|id| {
                let element = element(id, "t", &id.to_uppercase(), None);
                (element.id().clone(), element)
            })
            .collect(),
        boards: boards
            .into_iter()
            .map(|board| (board.id.clone(), board))
            .collect(),
    }
}

fn outline_of_board(snapshot: &Snapshot, board: &str) -> String {
    outline(snapshot, &board.parse().unwrap(), Geometry::Hidden, None).unwrap()
}

#[test]
fn shows_whose_inside_a_board_is_and_which_elements_open_an_inner_board() {
    let containers = Board {
        summary: Some("What the shop runs".to_owned()),
        ..hierarchy_board("containers", Some("shop"), &["payment", "db"])
    };
    let snapshot = hierarchy(
        vec![
            hierarchy_board("context", None, &["shop"]),
            containers,
            hierarchy_board("components", Some("payment"), &["api"]),
        ],
        &["shop", "payment", "db", "api"],
    );

    assert_eq!(
        outline_of_board(&snapshot, "containers"),
        "Board: containers · containers\n\
         Inside of: shop · Above: context\n\
         Summary: What the shop runs\n\
         \n\
         db       t  DB\n\
         payment  t  PAYMENT    ▸ components\n"
    );
}

#[rstest]
#[case::a_summary_alone(None, Some("S"), "Board: b · b\nSummary: S\n\n")]
#[case::an_owner_alone(Some("x"), None, "Board: b · b\nInside of: x · Above: top\n\n")]
#[case::an_owner_and_then_the_summary(
    Some("x"),
    Some("S"),
    "Board: b · b\nInside of: x · Above: top\nSummary: S\n\n"
)]
#[case::an_empty_summary_as_none(None, Some(""), "Board: b · b\n\n")]
fn writes_the_header_lines_in_order(
    #[case] owner: Option<&str>,
    #[case] summary: Option<&str>,
    #[case] expected: &str,
) {
    let board = Board {
        summary: summary.map(str::to_owned),
        ..hierarchy_board("b", owner, &[])
    };
    let snapshot = hierarchy(vec![board, hierarchy_board("top", None, &["x"])], &["x"]);

    assert_eq!(outline_of_board(&snapshot, "b"), expected);
}

#[test]
fn escapes_the_title_and_the_summary_onto_their_lines() {
    let board = Board {
        title: "T\r\nBoard: forged\t\\".to_owned(),
        summary: Some(
            "x\nInside of: evil · Above: main\u{2028}\u{202e}\u{200b}\u{1b}[31m".to_owned(),
        ),
        ..hierarchy_board("b", Some("x"), &[])
    };
    let snapshot = hierarchy(vec![board, hierarchy_board("top", None, &["x"])], &["x"]);

    assert_eq!(
        outline_of_board(&snapshot, "b"),
        "Board: b · T\\r\\nBoard: forged\\t\\\\\n\
         Inside of: x · Above: top\n\
         Summary: x\\nInside of: evil · Above: main\\u{2028}\\u{202e}\\u{200b}\\u{1b}[31m\n\
         \n"
    );
}

proptest! {
    #[test]
    fn writes_each_header_value_on_its_own_line_without_any_escaped_character(
        title in any::<String>(),
        summary in any::<String>(),
    ) {
        let board = Board {
            title,
            summary: Some(summary.clone()),
            ..hierarchy_board("b", Some("x"), &[])
        };
        let snapshot = hierarchy(vec![board, hierarchy_board("top", None, &["x"])], &["x"]);
        let expected = 3 + usize::from(!summary.is_empty());

        let outline = outline_of_board(&snapshot, "b");

        prop_assert_eq!(outline.lines().count(), expected);
        prop_assert_eq!(outline.matches('\n').count(), expected);
        prop_assert!(
            !outline.replace('\n', "").chars().any(hides_or_breaks_text),
            "{:?}",
            outline
        );
    }
}

/// Boards that hand edits and merges left in every state the owner line
/// marks: owners without a file, with and without a home; an owner without
/// a home; an owner placed on two boards; a board whose owner lies on it,
/// also an owner without a file; cycles of three and of two boards; and a
/// board below a cycle.
fn damaged() -> Snapshot {
    hierarchy(
        vec![
            hierarchy_board("top", None, &["placed-gone", "twice"]),
            hierarchy_board("alpha", None, &["twice"]),
            hierarchy_board("orphan", Some("gone"), &[]),
            hierarchy_board("orphan-placed", Some("placed-gone"), &[]),
            hierarchy_board("homeless", Some("loner"), &[]),
            hierarchy_board("doubled", Some("twice"), &[]),
            hierarchy_board("itself", Some("mirror"), &["mirror"]),
            hierarchy_board("ghost-loop", Some("ghost"), &["ghost"]),
            hierarchy_board("one", Some("x"), &["z", "w"]),
            hierarchy_board("two", Some("y"), &["x"]),
            hierarchy_board("three", Some("z"), &["y"]),
            hierarchy_board("below", Some("w"), &[]),
            hierarchy_board("left", Some("r"), &["l"]),
            hierarchy_board("right", Some("l"), &["r"]),
        ],
        &["twice", "loner", "mirror", "x", "y", "z", "w", "l", "r"],
    )
}

#[rstest]
#[case::an_owner_with_neither_file_nor_home("orphan", "Inside of: gone ⚠ missing")]
#[case::an_owner_without_a_file_that_a_board_places(
    "orphan-placed",
    "Inside of: placed-gone ⚠ missing · Above: top"
)]
#[case::an_owner_without_a_home("homeless", "Inside of: loner ⚠ no home")]
#[case::an_owner_on_two_boards_below_the_lower_id("doubled", "Inside of: twice · Above: alpha")]
#[case::a_board_whose_owner_lies_on_it("itself", "Inside of: mirror · Above: itself ⚠ cycle")]
#[case::a_board_whose_owner_without_a_file_lies_on_it(
    "ghost-loop",
    "Inside of: ghost ⚠ missing · Above: ghost-loop ⚠ cycle"
)]
#[case::the_first_board_of_a_cycle_of_three("one", "Inside of: x · Above: two ⚠ cycle")]
#[case::the_second_board_of_a_cycle_of_three("two", "Inside of: y · Above: three ⚠ cycle")]
#[case::the_third_board_of_a_cycle_of_three("three", "Inside of: z · Above: one ⚠ cycle")]
#[case::a_board_below_a_cycle("below", "Inside of: w · Above: one")]
#[case::one_board_of_a_cycle_of_two("left", "Inside of: r · Above: right ⚠ cycle")]
#[case::the_other_board_of_a_cycle_of_two("right", "Inside of: l · Above: left ⚠ cycle")]
fn marks_every_owner_and_board_that_breaks_the_hierarchy(
    #[case] board: &str,
    #[case] expected: &str,
) {
    let outline = outline_of_board(&damaged(), board);

    assert_eq!(outline.lines().nth(1), Some(expected), "{outline}");
}

/// The board `top` placing `twice`, the owner of `first` and `second`, and
/// `plain`, which owns no board.
fn twice_owned() -> Snapshot {
    hierarchy(
        vec![
            hierarchy_board("top", None, &["twice", "plain"]),
            hierarchy_board("second", Some("twice"), &[]),
            hierarchy_board("first", Some("twice"), &[]),
        ],
        &["twice", "plain"],
    )
}

#[test]
fn shows_both_boards_of_an_owner_of_two_on_its_line() {
    let snapshot = twice_owned();
    let ids: Vec<ElementId> = ["twice", "plain"]
        .iter()
        .map(|id| id.parse().unwrap())
        .collect();

    assert_eq!(
        outline_of_board(&snapshot, "top"),
        "Board: top · top\n\
         \n\
         plain  t  PLAIN\n\
         twice  t  TWICE    ▸ first, second\n"
    );
    assert_eq!(
        element_list(&snapshot, &ids, None),
        "twice  t  TWICE    ▸ first, second\n\
         plain  t  PLAIN\n"
    );
    assert_eq!(
        outline_of_board(&snapshot, "first").lines().nth(1),
        Some("Inside of: twice · Above: top")
    );
    assert_eq!(
        outline_of_board(&snapshot, "second").lines().nth(1),
        Some("Inside of: twice · Above: top")
    );
}

#[rstest]
#[case::two_boards("twice", "Boards: top\nInner board: first, second\n")]
#[case::none("plain", "Boards: top\nInner board: none\n")]
fn names_every_inner_board_in_the_detail(#[case] id: &str, #[case] expected: &str) {
    let detail = detail_of(&twice_owned(), id);

    assert!(detail.contains(expected), "{detail}");
}

#[test]
fn ends_no_line_in_spaces_when_an_empty_summary_ends_it() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", Some(""))], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(outline_of(&snapshot), "Board: b · Board\n\na  t  A\n");
    assert_eq!(element_list(&snapshot, [&id], None), "a  t  A\n");
}

/// The snapshot and review of [`marked`], with `inside-a`, the inside of
/// `a`, and `inside-c`, the inside of `c`.
fn marked_with_inner_boards() -> (Snapshot, Review) {
    let (mut snapshot, review) = marked();
    for (board, owner) in [("inside-a", "a"), ("inside-c", "c")] {
        let inside = hierarchy_board(board, Some(owner), &[]);
        snapshot.boards.insert(inside.id.clone(), inside);
    }
    (snapshot, review)
}

#[test]
fn shows_the_inner_boards_before_the_marker() {
    let (snapshot, review) = marked_with_inner_boards();

    let outline = outline(
        &snapshot,
        &"b".parse().unwrap(),
        Geometry::Hidden,
        Some(&review),
    )
    .unwrap();

    assert_eq!(
        outline,
        "Board: b · Board\n\
         \n\
         a  t  A     ▸ inside-a  ⚠ task\n\
         b  t  B  S              ⚠ hint\n\
         c  t  C     ▸ inside-c\n\
         \n\
         b → a  uses\n"
    );
}

#[test]
fn reports_the_inner_boards_of_the_elements_to_review() {
    let (snapshot, review) = marked_with_inner_boards();

    assert_eq!(
        status_report(&snapshot, &review),
        "Elements to review: 1 with a task, 1 with only a hint.\n\
         a  t  A     ▸ inside-a  ⚠ task\n\
         b  t  B  S              ⚠ hint\n"
    );
}

#[test]
fn renders_the_boards_as_a_tree() {
    let context = Board {
        title: "Context".to_owned(),
        summary: Some("The shop and its users".to_owned()),
        ..hierarchy_board("context", None, &["shop"])
    };
    let containers = Board {
        title: "Containers".to_owned(),
        summary: Some("What the shop runs".to_owned()),
        ..hierarchy_board("containers", Some("shop"), &["payment", "db"])
    };
    let components = Board {
        title: "Components".to_owned(),
        ..hierarchy_board("components", Some("payment"), &["api"])
    };
    let snapshot = hierarchy(
        vec![context, containers, components],
        &["shop", "payment", "db", "api"],
    );

    assert_eq!(
        overview(&snapshot, None),
        "context         Context     The shop and its users  1 element\n  \
         containers    Containers  What the shop runs      2 elements\n    \
         components  Components                          1 element\n"
    );
}

#[test]
fn renders_no_boards_as_a_sentence() {
    assert_eq!(overview(&Snapshot::default(), None), "No boards.");
}

#[test]
fn lists_the_boards_out_of_the_tree_at_the_top_level_with_their_owner_lines() {
    let overview = overview(&damaged(), None);

    let first_columns: Vec<&str> = overview
        .lines()
        .map(|line| {
            line.split("  ")
                .find(|column| !column.is_empty())
                .unwrap_or("")
        })
        .collect();
    let line_of = |board: &str| {
        overview
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("{board} ")))
            .unwrap_or("")
    };
    assert_eq!(
        first_columns,
        [
            "alpha",
            "doubled",
            "ghost-loop",
            "homeless",
            "itself",
            "left",
            "one",
            "below",
            "orphan",
            "orphan-placed",
            "right",
            "three",
            "top",
            "two"
        ]
    );
    assert!(line_of("doubled").starts_with("  doubled "), "{overview}");
    assert!(line_of("below").starts_with("  below "), "{overview}");
    for (board, owner_line) in [
        (
            "ghost-loop",
            "Inside of: ghost ⚠ missing · Above: ghost-loop ⚠ cycle",
        ),
        ("homeless", "Inside of: loner ⚠ no home"),
        ("one", "Inside of: x · Above: two ⚠ cycle"),
        ("orphan", "Inside of: gone ⚠ missing"),
        (
            "orphan-placed",
            "Inside of: placed-gone ⚠ missing · Above: top",
        ),
    ] {
        assert!(line_of(board).ends_with(owner_line), "{overview}");
    }
    assert!(!line_of("doubled").contains("Inside of"), "{overview}");
    assert!(!line_of("below").contains("Inside of"), "{overview}");
}

#[test]
fn counts_only_the_elements_with_a_file_whose_home_a_board_is() {
    let overview = overview(&damaged(), None);

    // `top` places `twice`, whose home is `alpha`, and `placed-gone`, which
    // has no file.
    let counts: Vec<(&str, &str)> = overview
        .lines()
        .filter_map(|line| {
            let columns: Vec<&str> = line
                .split("  ")
                .filter(|column| !column.is_empty())
                .collect();
            Some((*columns.first()?, *columns.get(2)?))
        })
        .filter(|(board, _)| ["alpha", "top", "one"].contains(&board.trim()))
        .collect();
    assert_eq!(
        counts,
        [
            ("alpha", "1 element"),
            ("one", "2 elements"),
            ("top", "0 elements")
        ]
    );
}

#[test]
fn counts_the_flags_of_each_board_s_own_elements_by_priority() {
    let (mut snapshot, review) = marked();
    // `a`, with a task, has its home on `aa` now, which sorts before `b`.
    let aa = hierarchy_board("aa", None, &["a"]);
    snapshot.boards.insert(aa.id.clone(), aa);

    assert_eq!(
        overview(&snapshot, Some(&review)),
        "aa  aa       1 element   ⚠ 1 task\n\
         b   Board    2 elements  ⚠ 1 hint\n"
    );
}

#[test]
fn escapes_the_title_and_the_summary_in_the_overview() {
    let board = Board {
        title: "T\r\nx\t\\".to_owned(),
        summary: Some("s\u{2028}\u{202e}\n".to_owned()),
        ..hierarchy_board("b", None, &[])
    };
    let snapshot = hierarchy(vec![board], &[]);

    assert_eq!(
        overview(&snapshot, None),
        "b  T\\r\\nx\\t\\\\  s\\u{2028}\\u{202e}\\n  0 elements\n"
    );
}

#[test]
fn lists_at_most_fifty_boards_and_pads_only_by_those() {
    let boards: Vec<Board> = (0..51)
        .map(|index| Board {
            title: if index == 50 {
                "A title far longer than any other on these boards".to_owned()
            } else {
                "T".to_owned()
            },
            ..hierarchy_board(&format!("b{index:02}"), None, &[])
        })
        .collect();

    let overview = overview(&hierarchy(boards, &[]), None);

    let lines: Vec<&str> = overview.lines().collect();
    assert_eq!(lines.len(), 51, "{overview}");
    assert_eq!(lines.first(), Some(&"b00  T    0 elements"));
    assert_eq!(lines.get(49), Some(&"b49  T    0 elements"));
    assert_eq!(lines.last(), Some(&"… and 1 more board"));
}

#[test]
fn renders_the_overview_of_a_cycle_of_ten_thousand_boards() {
    // Board `b{n}` is the inside of `e{n}`, which lies on `b{n+1}`, and
    // `e9999` lies on `b0`.
    let boards: Vec<Board> = (0..10_000_usize)
        .map(|index| {
            let below = format!("e{}", (index + 9_999) % 10_000);
            hierarchy_board(&format!("b{index}"), Some(&format!("e{index}")), &[&below])
        })
        .collect();
    let elements: Vec<String> = (0..10_000).map(|index| format!("e{index}")).collect();
    let elements: Vec<&str> = elements.iter().map(String::as_str).collect();

    let overview = overview(&hierarchy(boards, &elements), None);

    let lines: Vec<&str> = overview.lines().collect();
    assert_eq!(lines.len(), 51);
    assert!(
        lines.first().is_some_and(|line| line.ends_with("⚠ cycle")),
        "{overview}"
    );
    assert_eq!(lines.last(), Some(&"… and 9950 more boards"));
}

#[rstest]
#[case::a_title_at_the_limit(&"t".repeat(80), None, &"t".repeat(80), "")]
#[case::a_title_past_the_limit(&"t".repeat(81), None, &format!("{}…", "t".repeat(80)), "")]
#[case::a_summary_of_wide_characters_past_the_limit(
    "T",
    Some("é".repeat(161)),
    "T",
    &format!("{}…", "é".repeat(160))
)]
#[case::a_summary_cut_after_a_line_feed(
    "T",
    Some(format!("{}\ny", "s".repeat(159))),
    "T",
    &format!("{}\\n…", "s".repeat(159))
)]
fn cuts_long_titles_and_summaries_in_the_overview_before_escaping(
    #[case] title: &str,
    #[case] summary: Option<String>,
    #[case] expected_title: &str,
    #[case] expected_summary: &str,
) {
    let board = Board {
        title: title.to_owned(),
        summary,
        ..hierarchy_board("b", None, &[])
    };

    let overview = overview(&hierarchy(vec![board], &[]), None);

    let columns: Vec<&str> = overview
        .trim_end()
        .split("  ")
        .filter(|column| !column.is_empty())
        .collect();
    let expected: Vec<&str> = ["b", expected_title, expected_summary, "0 elements"]
        .into_iter()
        .filter(|column| !column.is_empty())
        .collect();
    assert_eq!(columns, expected);
}

#[test]
fn keeps_the_overview_small_whatever_the_size_of_titles_and_summaries() {
    let boards: Vec<Board> = (0..50)
        .map(|index| Board {
            title: "\u{202e}".repeat(10_000),
            summary: Some("\u{1b}".repeat(10_000)),
            ..hierarchy_board(&format!("b{index:02}"), None, &[])
        })
        .collect();

    let overview = overview(&hierarchy(boards, &[]), None);

    // Escaped in full, the texts would take 50 × 140,000 bytes.
    assert_eq!(overview.lines().count(), 50);
    assert!(overview.len() < 100_000, "{} bytes", overview.len());
}

/// `board`, a zone of `mode` and `statement`.
fn zoned(board: Board, mode: Mode, statement: StatementKind) -> Board {
    Board {
        zone: Some(Zone { mode, statement }),
        ..board
    }
}

/// `a-ideas`, a zone of ideas, places `y`; `a-plain`, no zone, places `x`;
/// `z-current`, a zone of current content, places `w`, `x`, and `y`, but is
/// the home of `w` alone.
fn zones() -> Snapshot {
    hierarchy(
        vec![
            zoned(
                hierarchy_board("a-ideas", None, &["y"]),
                Mode::Free,
                StatementKind::Idea,
            ),
            hierarchy_board("a-plain", None, &["x"]),
            zoned(
                hierarchy_board("z-current", None, &["w", "x", "y"]),
                Mode::Mirrored,
                StatementKind::Current,
            ),
        ],
        &["w", "x", "y"],
    )
}

#[test]
fn shows_each_element_s_own_kind_on_a_zone_and_never_the_zone_s() {
    assert_eq!(
        outline_of_board(&zones(), "z-current"),
        "Board: z-current · z-current\n\
         Zone: mirrored · current\n\
         \n\
         w  t  current  W\n\
         x  t  none     X\n\
         y  t  idea     Y\n"
    );
}

#[test]
fn shows_the_kind_column_on_a_zone_whose_elements_all_lie_at_home_elsewhere() {
    let snapshot = hierarchy(
        vec![
            hierarchy_board("a-plain", None, &["x"]),
            zoned(
                hierarchy_board("z-current", None, &["x"]),
                Mode::Mirrored,
                StatementKind::Current,
            ),
        ],
        &["x"],
    );

    assert_eq!(
        outline_of_board(&snapshot, "z-current").lines().last(),
        Some("x  t  none  X")
    );
}

#[test]
fn writes_none_before_the_name_of_an_element_without_a_kind() {
    let mut snapshot = zones();
    let named = element("x", "t", "current Payment flow", None);
    snapshot.elements.insert(named.id().clone(), named);
    let ids: Vec<ElementId> = ["y", "x"].iter().map(|id| id.parse().unwrap()).collect();

    assert_eq!(
        element_list(&snapshot, &ids, None),
        "y  t  idea  Y\n\
         x  t  none  current Payment flow\n"
    );
}

#[test]
fn shows_no_kind_column_in_a_list_without_kinds() {
    let snapshot = zones();
    let id: ElementId = "x".parse().unwrap();

    assert_eq!(element_list(&snapshot, [&id], None), "x  t  X\n");
}

#[test]
fn shows_the_zone_of_each_board_in_the_overview_and_no_zone_for_the_others() {
    let mut snapshot = zones();
    if let Some(board) = snapshot.boards.get_mut(&"a-plain".parse().unwrap()) {
        board.summary = Some("mirrored · current".to_owned());
    }

    let overview = overview(&snapshot, None);

    assert_eq!(
        overview.lines().collect::<Vec<_>>(),
        [
            format!(
                "a-ideas    a-ideas    free · idea{}1 element",
                " ".repeat(29)
            ),
            format!(
                "a-plain    a-plain    no zone{}mirrored · current  1 element",
                " ".repeat(13)
            ),
            format!(
                "z-current  z-current  mirrored · current{}1 element",
                " ".repeat(22)
            ),
        ]
    );
}

#[rstest]
#[case::at_home_on_the_lower_of_two_boards(
    "x",
    "Statement kind: current, from board alpha\n",
    "Boards: alpha, beta\n"
)]
#[case::without_a_home("loose", "Statement kind: none\n", "Boards: none\n")]
fn names_the_kind_and_the_home_board_it_comes_from_in_the_detail(
    #[case] id: &str,
    #[case] expected: &str,
    #[case] boards: &str,
) {
    let snapshot = hierarchy(
        vec![
            zoned(
                hierarchy_board("alpha", None, &["x"]),
                Mode::Mirrored,
                StatementKind::Current,
            ),
            zoned(
                hierarchy_board("beta", None, &["x"]),
                Mode::Free,
                StatementKind::Idea,
            ),
        ],
        &["x", "loose"],
    );

    let detail = detail_of(&snapshot, id);

    assert!(
        detail.contains(&format!("\nType: t\n{expected}")),
        "{detail}"
    );
    assert!(detail.contains(&format!("\n{boards}")), "{detail}");
}

#[test]
fn writes_the_zone_line_between_the_owner_and_the_summary_and_nothing_text_can_forge() {
    let board = Board {
        title: "T\nZone: free · idea".to_owned(),
        summary: Some("S\nZone: free · idea".to_owned()),
        ..zoned(
            hierarchy_board("b", Some("x"), &[]),
            Mode::Mirrored,
            StatementKind::Current,
        )
    };
    let snapshot = hierarchy(vec![board, hierarchy_board("top", None, &["x"])], &["x"]);

    assert_eq!(
        outline_of_board(&snapshot, "b"),
        "Board: b · T\\nZone: free · idea\n\
         Inside of: x · Above: top\n\
         Zone: mirrored · current\n\
         Summary: S\\nZone: free · idea\n\
         \n"
    );
}

/// `element(id, "t", id, None)` holding `kind` itself.
fn holding(id: &str, kind: StatementKind) -> Element {
    let mut fields = element(id, "t", id, None).into_fields();
    fields.statement = Some(kind);
    Element::new(id.parse().unwrap(), fields).unwrap()
}

#[test]
fn read_element_shows_an_own_kind_from_the_element_also_on_a_zone() {
    let board = zoned(
        hierarchy_board("b", None, &["x"]),
        Mode::Mirrored,
        StatementKind::Current,
    );
    let mut snapshot = hierarchy(vec![board], &["x"]);
    snapshot
        .elements
        .insert("x".parse().unwrap(), holding("x", StatementKind::Question));

    assert!(
        detail_of(&snapshot, "x").contains("\nStatement kind: question, from the element\n"),
        "{}",
        detail_of(&snapshot, "x")
    );
}

#[test]
fn read_element_shows_an_own_kind_without_a_home() {
    let mut snapshot = hierarchy(Vec::new(), &[]);
    snapshot
        .elements
        .insert("x".parse().unwrap(), holding("x", StatementKind::Current));

    let detail = detail_of(&snapshot, "x");

    assert!(
        detail.contains("\nStatement kind: current, from the element\n"),
        "{detail}"
    );
    assert!(detail.contains("\nBoards: none\n"), "{detail}");
}

#[test]
fn read_element_keeps_the_line_of_a_kind_from_a_board() {
    let board = zoned(
        hierarchy_board("b", None, &["x"]),
        Mode::Free,
        StatementKind::Idea,
    );
    let snapshot = hierarchy(vec![board], &["x"]);

    assert!(detail_of(&snapshot, "x").contains("\nStatement kind: idea, from board b\n"));
}

#[test]
fn no_text_can_forge_the_source_of_a_kind() {
    let forged = "N\nStatement kind: current, from the element";
    let mut fields = element("x", "t", forged, Some(forged)).into_fields();
    fields.body = "Statement kind: current, from the element\n".to_owned();
    let x = Element::new("x".parse().unwrap(), fields).unwrap();
    let mut snapshot = hierarchy(Vec::new(), &[]);
    snapshot.elements.insert("x".parse().unwrap(), x);

    let detail = detail_of(&snapshot, "x");

    let lines: Vec<&str> = detail.lines().collect();
    assert!(lines.contains(&"Statement kind: none"), "{detail}");
    let description = lines
        .iter()
        .position(|line| *line == "Description:")
        .unwrap();
    let forged_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == "Statement kind: current, from the element")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(forged_lines, [description + 1], "{detail}");
}

#[test]
fn the_kind_column_shows_an_own_kind_on_a_board_that_is_no_zone() {
    let mut snapshot = snapshot("T", vec![element("a", "t", "A", None)], &[]);
    let b = holding("b", StatementKind::Question);
    snapshot.elements.insert("b".parse().unwrap(), b);
    snapshot
        .boards
        .get_mut(&"b".parse().unwrap())
        .unwrap()
        .placements
        .insert(
            "b".parse().unwrap(),
            Placement {
                rect: rect(),
                pinned: false,
            },
        );

    let lines = outline_of(&snapshot);

    assert!(lines.contains("a  t  none      A\n"), "{lines}");
    assert!(lines.contains("b  t  question  b\n"), "{lines}");
}

#[test]
fn an_own_kind_on_a_zone_of_another_kind_shows_in_its_line_and_a_missing_file_shows_as_missing() {
    let board = zoned(
        hierarchy_board("b", None, &["ghost", "x"]),
        Mode::Mirrored,
        StatementKind::Current,
    );
    let mut snapshot = hierarchy(vec![board], &["x"]);
    snapshot
        .elements
        .insert("x".parse().unwrap(), holding("x", StatementKind::Idea));

    let lines = outline_of_board(&snapshot, "b");

    assert!(lines.contains("x      t  idea"), "{lines}");
    assert!(lines.contains("ghost  ⚠ missing"), "{lines}");
}

#[test]
fn an_own_kind_changes_no_review_flag() {
    let plain = stamped_snapshot();
    let mut own_kind = plain.clone();
    let id: ElementId = "x".parse().unwrap();
    let mut fields = own_kind.elements[&id].clone().into_fields();
    fields.statement = Some(StatementKind::Idea);
    own_kind
        .elements
        .insert(id.clone(), Element::new(id, fields).unwrap());
    let changed = facts(&[b"crates/x/a.rs"], &[b"crates/x/a.rs"]);

    let flagged = |snapshot: &Snapshot| {
        let review = review(snapshot, &changed);
        status_report(snapshot, &review)
    };

    assert!(flagged(&plain).contains("⚠ task"), "{}", flagged(&plain));
    assert!(
        flagged(&own_kind).contains("⚠ task"),
        "{}",
        flagged(&own_kind)
    );
    assert!(
        flagged(&own_kind).contains("idea"),
        "{}",
        flagged(&own_kind)
    );
}
