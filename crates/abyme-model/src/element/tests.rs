//! Tests of building, reading, and writing element files.

use std::collections::BTreeSet;

use rstest::rstest;

use super::*;
use crate::verification::{CommitId, ObjectId, RecordedContent, Verifier};

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn type_ref(value: &str) -> TypeRef {
    TypeRef::try_from(value.to_owned()).unwrap()
}

/// An element `main` with the given name and summary.
fn main_element(name: &str, summary: Option<&str>) -> Result<Element, ElementError> {
    Element::new(
        element_id("main"),
        ElementFields {
            statement: None,
            type_ref: type_ref("t"),
            name: name.to_owned(),
            summary: summary.map(str::to_owned),
            body: String::new(),
            relations: Relations::new(),
            anchor: None,
            verified: None,
        },
    )
}

/// The documented example, reduced to the fields of the skeleton.
fn payment_service() -> Element {
    Element::new(
        element_id("payment-service"),
        ElementFields {
            statement: None,
            type_ref: type_ref("c4/container"),
            name: "Payment Service".to_owned(),
            summary: Some("Payment processing through Stripe".to_owned()),
            body: "Handles payments through Stripe. Refunds run asynchronously\nthrough the refund-worker.\n"
                .to_owned(),
            relations: Relations::new(),
            anchor: None,
            verified: None,
        },
    )
    .unwrap()
}

/// An element file whose frontmatter starts with the valid lines for `main`.
fn main_file(extra_lines: &str) -> String {
    format!("---\nid: \"main\"\ntype: \"t\"\nname: \"Main\"\n{extra_lines}---\nbody\n")
}

fn read_main(text: &str) -> Result<Element, ElementError> {
    Element::from_markdown(&element_id("main"), text)
}

fn problem_of(text: &str) -> ElementProblem {
    read_main(text).unwrap_err().problem().clone()
}

/// The frontmatter lines of a written file.
fn frontmatter_of(markdown: &str) -> &str {
    let rest = markdown.strip_prefix("---\n").unwrap();
    &rest[..=rest.find("\n---\n").unwrap()]
}

#[test]
fn writes_the_documented_layout() {
    let expected = "---
id: \"payment-service\"
type: \"c4/container\"
name: \"Payment Service\"
summary: \"Payment processing through Stripe\"
---
Handles payments through Stripe. Refunds run asynchronously
through the refund-worker.
";

    assert_eq!(payment_service().to_markdown(), expected);
}

#[test]
fn reads_back_what_it_writes() {
    let element = payment_service();

    let read = Element::from_markdown(element.id(), &element.to_markdown()).unwrap();

    assert_eq!(read, element);
}

#[test]
fn rebuilds_the_same_element_from_its_fields() {
    let element = payment_service();

    let rebuilt = Element::new(element.id().clone(), element.clone().into_fields()).unwrap();

    assert_eq!(rebuilt, element);
}

#[test]
fn checks_changed_fields_again_when_rebuilding() {
    let element = payment_service();
    let fields = ElementFields {
        name: "Payment\u{0}Service".to_owned(),
        ..element.clone().into_fields()
    };

    let error = Element::new(element.id().clone(), fields).unwrap_err();

    assert_eq!(error.element(), element.id());
    assert_eq!(
        error.problem(),
        &ElementProblem::InvalidCharacter {
            field: "name",
            code: 0
        }
    );
}

#[test]
fn reads_plain_values_written_by_hand() {
    let text = "---
id: payment-service
type: c4/container
name: Payment Service
summary: Payment processing through Stripe
---
Handles payments through Stripe. Refunds run asynchronously
through the refund-worker.
";

    let read = Element::from_markdown(&element_id("payment-service"), text).unwrap();

    assert_eq!(read, payment_service());
}

#[test]
fn writes_no_key_for_an_absent_summary_and_reads_it_back_as_absent() {
    let element = main_element("Main", None).unwrap();

    let markdown = element.to_markdown();

    assert!(!markdown.contains("summary"));
    assert_eq!(read_main(&markdown).unwrap().summary(), None);
}

#[test]
fn keeps_an_empty_summary_apart_from_an_absent_one() {
    let element = main_element("Main", Some("")).unwrap();

    let markdown = element.to_markdown();

    assert!(markdown.contains("summary: \"\"\n"));
    assert_eq!(read_main(&markdown).unwrap().summary(), Some(""));
}

#[test]
fn reads_carriage_returns_and_keeps_the_body_byte_for_byte() {
    let text =
        "---\r\nid: \"main\"\r\ntype: \"t\"\r\nname: \"Main\"\r\n---\r\nline\r\n---\r\n  tail  ";

    let element = read_main(text).unwrap();

    assert_eq!(element.name(), "Main");
    assert_eq!(element.body(), "line\r\n---\r\n  tail  ");
}

#[test]
fn reads_a_closing_line_at_the_end_of_the_file() {
    let text = "---\nid: \"main\"\ntype: \"t\"\nname: \"Main\"\n---";

    assert_eq!(read_main(text).unwrap().body(), "");
}

#[rstest]
#[case::tab("a\tb")]
#[case::carriage_return("a\rb")]
#[case::line_feed("a\nb")]
#[case::double_quote("a\"b")]
#[case::backslash("a\\b")]
#[case::next_line("a\u{85}b")]
#[case::line_separator("a\u{2028}b")]
#[case::paragraph_separator("a\u{2029}b")]
#[case::byte_order_mark("a\u{feff}b")]
#[case::delimiter_line("a\n---\nb")]
fn writes_special_characters_escaped_and_reads_them_back(#[case] text: &str) {
    let element = main_element(text, Some(text)).unwrap();

    let markdown = element.to_markdown();

    let frontmatter = frontmatter_of(&markdown);
    let raw = ['\t', '\r', '\u{85}', '\u{2028}', '\u{2029}', '\u{feff}'];
    assert!(!frontmatter.contains(raw), "{frontmatter:?}");
    assert_eq!(frontmatter.lines().count(), 4);
    assert_eq!(read_main(&markdown).unwrap(), element);
}

#[rstest]
#[case::missing_id("type: \"t\"\nname: \"Main\"\n", ElementProblem::MissingKey { key: "id" })]
#[case::missing_type("id: \"main\"\nname: \"Main\"\n", ElementProblem::MissingKey { key: "type" })]
#[case::missing_name("id: \"main\"\ntype: \"t\"\n", ElementProblem::MissingKey { key: "name" })]
#[case::empty_type(
    "id: \"main\"\ntype: \"\"\nname: \"Main\"\n",
    ElementProblem::EmptyType
)]
#[case::not_a_mapping("- a\n", ElementProblem::NotAMapping)]
#[case::empty_frontmatter("", ElementProblem::DocumentCount { count: 0 })]
fn rejects_frontmatter_without_the_required_fields(
    #[case] frontmatter: &str,
    #[case] problem: ElementProblem,
) {
    let text = format!("---\n{frontmatter}---\nbody\n");

    assert_eq!(problem_of(&text), problem);
}

#[rstest]
#[case::unknown_key("statements: \"a\"\n", ElementProblem::UnknownKey { key: "\"statements\"".to_owned() })]
#[case::statement_as_a_list("statement: [current]\n", ElementProblem::NonScalarValue { key: "\"statement\"".to_owned() })]
#[case::statement_as_a_mapping("statement: {idea: x}\n", ElementProblem::NonScalarValue { key: "\"statement\"".to_owned() })]
#[case::statement_twice(
    "statement: \"idea\"\nstatement: \"idea\"\n",
    ElementProblem::DuplicateKey { key: "\"statement\"".to_owned() }
)]
#[case::statement_twice_quoted(
    "statement: \"idea\"\n\"statement\": \"idea\"\n",
    ElementProblem::DuplicateKey { key: "\"statement\"".to_owned() }
)]
#[case::tag_on_statement("statement: !!str current\n", ElementProblem::Tag)]
#[case::anchor_on_statement("statement: &a current\n", ElementProblem::Anchor)]
#[case::duplicate_key_quoted_and_plain(
    "\"id\": \"other\"\n",
    ElementProblem::DuplicateKey { key: "\"id\"".to_owned() }
)]
#[case::list_value("summary: [a]\n", ElementProblem::NonScalarValue { key: "\"summary\"".to_owned() })]
#[case::mapping_value("summary: {a: b}\n", ElementProblem::NonScalarValue { key: "\"summary\"".to_owned() })]
#[case::list_key("? [a]\n: b\n", ElementProblem::NonScalarKey)]
#[case::flow_mapping_key("{a: b}: c\n", ElementProblem::NonScalarKey)]
#[case::tag_on_value("summary: !!str a\n", ElementProblem::Tag)]
#[case::local_tag_on_value("summary: ! a\n", ElementProblem::Tag)]
#[case::tag_on_key("!!str summary: a\n", ElementProblem::Tag)]
#[case::local_tag_on_key("! summary: a\n", ElementProblem::Tag)]
#[case::anchor_on_value("summary: &x a\n", ElementProblem::Anchor)]
#[case::anchor_on_key("&x summary: a\n", ElementProblem::Anchor)]
#[case::document_end_marker("...\n", ElementProblem::DocumentMarker)]
#[case::second_document("...\nid: \"x\"\n", ElementProblem::DocumentCount { count: 2 })]
fn rejects_frontmatter_beyond_a_flat_mapping(
    #[case] extra_lines: &str,
    #[case] problem: ElementProblem,
) {
    assert_eq!(problem_of(&main_file(extra_lines)), problem);
}

#[rstest]
#[case::alias_as_value("id: \"main\"\ntype: \"t\"\nname: *x\n")]
#[case::alias_as_key("id: \"main\"\ntype: \"t\"\nname: \"Main\"\n*x : a\n")]
fn rejects_aliases(#[case] frontmatter: &str) {
    let text = format!("---\n{frontmatter}---\n");

    assert_eq!(problem_of(&text), ElementProblem::Alias);
}

#[rstest]
#[case::document_start_marker(
    "--- \nid: \"main\"\ntype: \"t\"\nname: \"Main\"\n",
    ElementProblem::DocumentMarker
)]
#[case::document_start_marker_with_a_tag(
    "--- !!map\nid: \"main\"\ntype: \"t\"\nname: \"Main\"\n",
    ElementProblem::DocumentMarker
)]
#[case::yaml_directive(
    "%YAML 1.2\n--- \nid: \"main\"\ntype: \"t\"\nname: \"Main\"\n",
    ElementProblem::Directive
)]
#[case::tag_directive(
    "%TAG !e! tag:example.com,2000:\n--- \nid: \"main\"\ntype: \"t\"\nname: \"Main\"\n",
    ElementProblem::Directive
)]
#[case::lone_carriage_return_before_a_marker(
    "id: \"main\"\ntype: \"t\"\nname: \"Main\"\n---\rsummary: \"a\"\n",
    ElementProblem::DocumentCount { count: 2 }
)]
fn rejects_markers_and_directives_inside_the_frontmatter(
    #[case] frontmatter: &str,
    #[case] problem: ElementProblem,
) {
    let text = format!("---\n{frontmatter}---\n");

    assert_eq!(problem_of(&text), problem);
}

#[test]
fn rejects_a_tag_on_the_root_mapping_without_a_marker() {
    let text = "---\n!!map {id: main, type: t, name: Main}\n---\n";

    assert_eq!(problem_of(text), ElementProblem::Tag);
}

#[rstest]
#[case::path_traversal("../x")]
#[case::uppercase("Main")]
fn rejects_an_invalid_id(#[case] id: &str) {
    let text = format!("---\nid: \"{id}\"\ntype: \"t\"\nname: \"Main\"\n---\n");

    assert!(matches!(
        problem_of(&text),
        ElementProblem::InvalidId { .. }
    ));
}

#[test]
fn rejects_a_file_that_names_another_element() {
    let text = "---\nid: \"other\"\ntype: \"t\"\nname: \"Main\"\n---\n";

    assert_eq!(
        problem_of(text),
        ElementProblem::IdMismatch {
            found: element_id("other"),
        }
    );
}

#[rstest]
#[case::start_of_heading('\u{1}')]
#[case::escape('\u{1b}')]
#[case::delete('\u{7f}')]
#[case::first_c1_control('\u{80}')]
#[case::last_c1_control('\u{9f}')]
#[case::noncharacter_fffe('\u{fffe}')]
#[case::noncharacter_ffff('\u{ffff}')]
fn rejects_a_character_the_parser_cannot_read_back(#[case] character: char) {
    let text = format!("a{character}b");
    let expected = |field| ElementProblem::InvalidCharacter {
        field,
        code: u32::from(character),
    };

    let as_name = main_element(&text, None).unwrap_err();
    let as_summary = main_element("Main", Some(&text)).unwrap_err();
    let as_type = TypeRef::try_from(text.clone()).unwrap_err();
    let raw_in_file = read_main(&main_file(&format!("summary: \"{text}\"\n")));
    let escaped_in_file = read_main(&main_file(&format!(
        "summary: \"a\\u{:04X}b\"\n",
        u32::from(character)
    )));

    assert_eq!(as_name.problem(), &expected("name"));
    assert_eq!(as_summary.problem(), &expected("summary"));
    assert_eq!(as_type, expected("type"));
    for read in [raw_in_file, escaped_in_file] {
        let problem = read.unwrap_err().problem().clone();
        assert!(
            matches!(
                problem,
                ElementProblem::InvalidCharacter { .. } | ElementProblem::Yaml { .. }
            ),
            "{problem:?}"
        );
    }
}

#[test]
fn accepts_a_frontmatter_of_exactly_the_limit() {
    // `id: "main"` + `type: "t"` + `name: ""` with line feeds take 30 bytes.
    let name = "a".repeat(MAX_FRONTMATTER_BYTES - 30);

    let element = main_element(&name, None).unwrap();

    assert_eq!(
        frontmatter_of(&element.to_markdown()).len(),
        MAX_FRONTMATTER_BYTES
    );
    assert_eq!(read_main(&element.to_markdown()).unwrap(), element);
}

#[test]
fn rejects_an_element_whose_frontmatter_exceeds_the_limit_by_one_byte() {
    let name = "a".repeat(MAX_FRONTMATTER_BYTES - 29);

    let error = main_element(&name, None).unwrap_err();

    assert_eq!(
        error.problem(),
        &ElementProblem::FrontmatterTooLarge {
            size: MAX_FRONTMATTER_BYTES + 1,
        }
    );
}

#[test]
fn rejects_a_file_over_the_limit_before_parsing_it() {
    let text = format!("---\n{}\n---\n", "[".repeat(MAX_FRONTMATTER_BYTES));

    assert_eq!(
        problem_of(&text),
        ElementProblem::FrontmatterTooLarge {
            size: MAX_FRONTMATTER_BYTES + 1,
        }
    );
}

#[test]
fn counts_carriage_return_line_feeds_once_against_the_limit() {
    let name = "a".repeat(MAX_FRONTMATTER_BYTES - 30);
    let text = format!("---\r\nid: \"main\"\r\ntype: \"t\"\r\nname: \"{name}\"\r\n---\r\n");

    assert_eq!(read_main(&text).unwrap().name(), name);
}

#[test]
fn rejects_a_file_whose_escapes_would_grow_past_the_limit_when_written() {
    let tabs = "\t".repeat(40_000);
    let text = main_file(&format!("summary: \"{tabs}\"\n"));

    assert!(matches!(
        problem_of(&text),
        ElementProblem::FrontmatterTooLarge { .. }
    ));
}

#[test]
fn error_escapes_control_characters_from_the_file() {
    let error = read_main(&main_file("\"a\\eb\\nc\": \"x\"\n")).unwrap_err();

    let message = error.to_string();

    assert!(!message.chars().any(char::is_control), "{message:?}");
    assert!(message.starts_with("element main: "), "{message}");
}

/// The element `main` with the given relations.
fn main_with_relations(
    relations: &[(RelationKind, &str, Option<&str>)],
) -> Result<Element, ElementError> {
    let relations = relations
        .iter()
        .map(|(kind, to, label)| ((*kind, element_id(to)), label.map(str::to_owned)))
        .collect();
    Element::new(
        element_id("main"),
        ElementFields {
            relations,
            ..main_element("Main", None).unwrap().into_fields()
        },
    )
}

#[test]
fn writes_relations_one_per_line_sorted_by_kind_name_then_target() {
    let element = main_with_relations(&[
        (RelationKind::Uses, "orders-db", Some("REST")),
        (RelationKind::DependsOn, "payment", None),
        (RelationKind::Uses, "checkout", None),
    ])
    .unwrap();

    let expected = "id: \"main\"
type: \"t\"
name: \"Main\"
relations:
  - {kind: \"depends_on\", to: \"payment\"}
  - {kind: \"uses\", to: \"checkout\"}
  - {kind: \"uses\", to: \"orders-db\", label: \"REST\"}
";
    assert_eq!(frontmatter_of(&element.to_markdown()), expected);
    assert_eq!(read_main(&element.to_markdown()).unwrap(), element);
}

#[test]
fn writes_no_relations_key_without_relations() {
    let element = main_element("Main", None).unwrap();

    assert!(!element.to_markdown().contains("relations"));
}

#[rstest]
#[case::quote_and_backslash("a\"b\\c")]
#[case::flow_indicators("a, b} {c] [d")]
#[case::colon_and_hash("a: b # c")]
#[case::line_breaks("a\nb\rc\u{85}d\u{2028}e\u{2029}f")]
#[case::tab_and_byte_order_mark("a\tb\u{feff}c")]
fn reads_back_labels_with_special_characters(#[case] label: &str) {
    let element = main_with_relations(&[(RelationKind::Uses, "x", Some(label))]).unwrap();

    let read = read_main(&element.to_markdown()).unwrap();

    assert_eq!(read, element);
}

#[test]
fn reads_relations_written_by_hand_in_any_style() {
    let text = main_file(
        "relations:\n- kind: uses\n  to: orders-db\n  label: REST\n- {kind: 'concerns', \"to\": x}\n",
    );

    let read = read_main(&text).unwrap();

    let expected = main_with_relations(&[
        (RelationKind::Uses, "orders-db", Some("REST")),
        (RelationKind::Concerns, "x", None),
    ])
    .unwrap();
    assert_eq!(read.relations(), expected.relations());
}

#[test]
fn reads_a_relation_whose_target_has_no_file() {
    let read = read_main(&main_file("relations: [{kind: uses, to: nowhere}]\n")).unwrap();

    assert_eq!(
        read.relations().keys().collect::<Vec<_>>(),
        [&(RelationKind::Uses, element_id("nowhere"))]
    );
}

#[test]
fn reads_an_empty_list_of_relations_as_none() {
    let read = read_main(&main_file("relations: []\n")).unwrap();

    assert_eq!(read.relations(), &Relations::new());
    assert!(!read.to_markdown().contains("relations"));
}

#[rstest]
#[case::anchor_on_the_list("relations: &a [{kind: uses, to: x}]\n", ElementProblem::Anchor)]
#[case::tag_on_the_list("relations: !!seq [{kind: uses, to: x}]\n", ElementProblem::Tag)]
#[case::alias_as_the_list("relations: *a\n", ElementProblem::Alias)]
#[case::anchor_on_a_relation("relations:\n  - &a {kind: uses, to: x}\n", ElementProblem::Anchor)]
#[case::tag_on_a_relation("relations:\n  - !!map {kind: uses, to: x}\n", ElementProblem::Tag)]
#[case::alias_as_a_relation("relations:\n  - *a\n", ElementProblem::Alias)]
#[case::anchor_on_a_relation_key("relations: [{&a kind: uses, to: x}]\n", ElementProblem::Anchor)]
#[case::tag_on_a_relation_key("relations: [{!!str kind: uses, to: x}]\n", ElementProblem::Tag)]
#[case::anchor_on_a_relation_value("relations: [{kind: &a uses, to: x}]\n", ElementProblem::Anchor)]
#[case::tag_on_a_relation_value("relations: [{kind: !!str uses, to: x}]\n", ElementProblem::Tag)]
#[case::alias_as_a_relation_value("relations: [{kind: uses, to: *a}]\n", ElementProblem::Alias)]
#[case::plain_value("relations: x\n", ElementProblem::RelationsNotAList)]
#[case::empty_value("relations:\n", ElementProblem::RelationsNotAList)]
#[case::mapping_value("relations: {kind: uses, to: x}\n", ElementProblem::RelationsNotAList)]
#[case::plain_relation("relations:\n  - x\n", ElementProblem::RelationNotAMapping)]
#[case::list_as_a_relation("relations:\n  - [uses, x]\n", ElementProblem::RelationNotAMapping)]
#[case::list_as_a_kind(
    "relations: [{kind: [uses], to: x}]\n",
    ElementProblem::NonScalarRelationValue { key: "\"kind\"".to_owned() }
)]
#[case::mapping_as_a_target(
    "relations: [{kind: uses, to: {a: b}}]\n",
    ElementProblem::NonScalarRelationValue { key: "\"to\"".to_owned() }
)]
#[case::list_as_a_label(
    "relations: [{kind: uses, to: x, label: [a]}]\n",
    ElementProblem::NonScalarRelationValue { key: "\"label\"".to_owned() }
)]
#[case::list_under_an_unknown_relation_key(
    "relations: [{kind: uses, to: x, color: [red]}]\n",
    ElementProblem::NonScalarRelationValue { key: "\"color\"".to_owned() }
)]
#[case::first_relation_fails_before_a_later_one(
    "relations: [{to: x}, {kind: [uses], to: y}]\n",
    ElementProblem::MissingRelationKey { key: "kind" }
)]
#[case::tag_on_a_plain_relation("relations:\n  - !!str x\n", ElementProblem::Tag)]
#[case::list_as_a_relation_key("relations: [{[a]: b}]\n", ElementProblem::NonScalarKey)]
#[case::unknown_relation_key(
    "relations: [{kind: uses, to: x, color: red}]\n",
    ElementProblem::UnknownRelationKey { key: "\"color\"".to_owned() }
)]
#[case::relation_key_quoted_and_plain(
    "relations: [{kind: uses, \"kind\": uses, to: x}]\n",
    ElementProblem::DuplicateRelationKey { key: "\"kind\"".to_owned() }
)]
#[case::missing_kind("relations: [{to: x}]\n", ElementProblem::MissingRelationKey { key: "kind" })]
#[case::missing_target("relations: [{kind: uses}]\n", ElementProblem::MissingRelationKey { key: "to" })]
#[case::capitalized_kind(
    "relations: [{kind: Uses, to: x}]\n",
    ElementProblem::UnknownRelationKind { kind: "\"Uses\"".to_owned() }
)]
#[case::kind_with_a_trailing_space(
    "relations: [{kind: \"uses \", to: x}]\n",
    ElementProblem::UnknownRelationKind { kind: "\"uses \"".to_owned() }
)]
#[case::kind_not_in_the_mvp(
    "relations: [{kind: contains, to: x}]\n",
    ElementProblem::UnknownRelationKind { kind: "\"contains\"".to_owned() }
)]
#[case::same_relation_in_flow_and_block_style(
    "relations:\n  - {kind: uses, to: x}\n  - kind: \"uses\"\n    to: \"x\"\n",
    ElementProblem::DuplicateRelation { kind: RelationKind::Uses, to: element_id("x") }
)]
#[case::relation_to_itself(
    "relations: [{kind: describes, to: main}]\n",
    ElementProblem::SelfRelation { kind: RelationKind::Describes }
)]
#[case::list_value_outside_relations("summary: [a]\nrelations: []\n", ElementProblem::NonScalarValue { key: "\"summary\"".to_owned() })]
#[case::relations_twice(
    "relations: []\nrelations: []\n",
    ElementProblem::DuplicateKey { key: "\"relations\"".to_owned() }
)]
fn rejects_relations_beyond_a_list_of_flat_mappings(
    #[case] extra_lines: &str,
    #[case] problem: ElementProblem,
) {
    assert_eq!(problem_of(&main_file(extra_lines)), problem);
}

#[rstest]
#[case::path_traversal("../x")]
#[case::uppercase("A")]
#[case::device_name("con")]
#[case::too_long("a234567890123456789012345678901234567890123456789012345678901234x")]
fn rejects_a_relation_to_an_invalid_id(#[case] to: &str) {
    let text = main_file(&format!("relations: [{{kind: uses, to: \"{to}\"}}]\n"));

    assert!(matches!(
        problem_of(&text),
        ElementProblem::InvalidRelationTarget { .. }
    ));
}

#[rstest]
#[case::null('\u{0}')]
#[case::escape('\u{1b}')]
#[case::noncharacter('\u{fffe}')]
fn rejects_a_label_character_without_showing_the_label(#[case] character: char) {
    let label = format!("secret{character}");

    let error = main_with_relations(&[(RelationKind::Uses, "x", Some(&label))]).unwrap_err();

    assert_eq!(
        error.problem(),
        &ElementProblem::InvalidCharacter {
            field: "label",
            code: u32::from(character)
        }
    );
    assert!(!error.to_string().contains("secret"), "{error}");
}

#[test]
fn rejects_a_label_character_in_a_file_without_showing_the_label() {
    let error = read_main(&main_file(
        "relations: [{kind: uses, to: x, label: \"secret\\0\"}]\n",
    ))
    .unwrap_err();

    assert!(!error.to_string().contains("secret"), "{error}");
}

#[test]
fn rejects_a_relation_to_itself_when_building() {
    let error = main_with_relations(&[(RelationKind::Uses, "main", None)]).unwrap_err();

    assert_eq!(
        error.problem(),
        &ElementProblem::SelfRelation {
            kind: RelationKind::Uses
        }
    );
}

#[test]
fn rejects_a_label_that_takes_the_frontmatter_past_the_limit() {
    let label = "a".repeat(MAX_FRONTMATTER_BYTES);

    let error = main_with_relations(&[(RelationKind::Uses, "x", Some(&label))]).unwrap_err();

    assert!(matches!(
        error.problem(),
        ElementProblem::FrontmatterTooLarge { .. }
    ));
}

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// The element `main` with the given anchor and stamp.
fn main_with_anchor_and_stamp(
    anchor: Option<&str>,
    stamp: Option<Verifier>,
) -> Result<Element, ElementError> {
    Element::new(
        element_id("main"),
        ElementFields {
            anchor: anchor.map(|anchor| Anchor::try_from(anchor.to_owned()).unwrap()),
            verified: stamp.map(|by| Verification {
                commit: CommitId::try_from(COMMIT.to_owned()).unwrap(),
                by,
                content: None,
            }),
            ..main_with_relations(&[(RelationKind::Uses, "x", None)])
                .unwrap()
                .into_fields()
        },
    )
}

#[test]
fn writes_anchor_and_stamp_after_the_summary_and_before_the_relations() {
    let element =
        main_with_anchor_and_stamp(Some("crates/payment-service"), Some(Verifier::Person)).unwrap();

    let expected = format!(
        "id: \"main\"\n\
         type: \"t\"\n\
         name: \"Main\"\n\
         anchor: \"crates/payment-service\"\n\
         verified: \"{COMMIT}\"\n\
         verified_by: \"person\"\n\
         relations:\n  \
         - {{kind: \"uses\", to: \"x\"}}\n"
    );
    assert_eq!(frontmatter_of(&element.to_markdown()), expected);
    assert_eq!(read_main(&element.to_markdown()).unwrap(), element);
}

#[rstest]
#[case::anchor_alone(Some("docs/api.md"), None)]
#[case::stamp_alone(None, Some(Verifier::Ai))]
#[case::both(Some("docs/api.md"), Some(Verifier::Ai))]
fn reads_back_an_anchor_and_a_stamp_with_or_without_each_other(
    #[case] anchor: Option<&str>,
    #[case] stamp: Option<Verifier>,
) {
    let element = main_with_anchor_and_stamp(anchor, stamp).unwrap();

    let read = read_main(&element.to_markdown()).unwrap();

    assert_eq!(read.anchor().map(Anchor::as_str), anchor);
    assert_eq!(read.verified().map(|stamp| stamp.by), stamp);
    assert_eq!(read, element);
}

#[test]
fn reads_anchor_and_stamp_written_by_hand() {
    let text = main_file(&format!(
        "anchor: crates/x\nverified: {COMMIT}\nverified_by: ai\n"
    ));

    let read = read_main(&text).unwrap();

    assert_eq!(read.anchor().map(Anchor::as_str), Some("crates/x"));
    assert_eq!(
        read.verified()
            .map(|stamp| (stamp.commit.as_str(), stamp.by)),
        Some((COMMIT, Verifier::Ai))
    );
}

#[rstest]
#[case::tilde("~")]
#[case::null("null")]
fn reads_a_yaml_null_word_as_an_anchor_and_writes_it_back_quoted(#[case] word: &str) {
    let read = read_main(&main_file(&format!("anchor: {word}\n"))).unwrap();

    assert_eq!(read.anchor().map(Anchor::as_str), Some(word));
    assert!(
        read.to_markdown()
            .contains(&format!("anchor: \"{word}\"\n"))
    );
    assert_eq!(read_main(&read.to_markdown()).unwrap(), read);
}

#[rstest]
#[case::empty_value("anchor:\n")]
#[case::empty_string("anchor: \"\"\n")]
fn rejects_an_empty_anchor(#[case] extra_lines: &str) {
    assert!(matches!(
        problem_of(&main_file(extra_lines)),
        ElementProblem::InvalidAnchor {
            rule: AnchorRule::Empty,
            ..
        }
    ));
}

#[rstest]
#[case::list_as_anchor(
    "anchor: [a]\n",
    ElementProblem::NonScalarValue { key: "\"anchor\"".to_owned() }
)]
#[case::mapping_as_anchor(
    "anchor: {a: b}\n",
    ElementProblem::NonScalarValue { key: "\"anchor\"".to_owned() }
)]
#[case::yaml_anchor_on_the_anchor("anchor: &x a\n", ElementProblem::Anchor)]
#[case::anchor_twice(
    "anchor: a\nanchor: b\n",
    ElementProblem::DuplicateKey { key: "\"anchor\"".to_owned() }
)]
#[case::anchor_quoted_and_plain(
    "anchor: a\n\"anchor\": b\n",
    ElementProblem::DuplicateKey { key: "\"anchor\"".to_owned() }
)]
#[case::commit_alone(
    "verified: \"0123456789abcdef0123456789abcdef01234567\"\n",
    ElementProblem::IncompleteStamp { missing: "verified_by" }
)]
#[case::verifier_alone(
    "verified_by: ai\n",
    ElementProblem::IncompleteStamp { missing: "verified" }
)]
#[case::uppercase_verifier(
    "verified: \"0123456789abcdef0123456789abcdef01234567\"\nverified_by: AI\n",
    ElementProblem::UnknownVerifier { verifier: "\"AI\"".to_owned() }
)]
#[case::abbreviated_commit(
    "verified: \"0123456\"\nverified_by: ai\n",
    ElementProblem::InvalidCommit { commit: "\"0123456\"".to_owned() }
)]
fn rejects_anchors_and_stamps_that_break_the_rules(
    #[case] extra_lines: &str,
    #[case] problem: ElementProblem,
) {
    assert_eq!(problem_of(&main_file(extra_lines)), problem);
}

#[test]
fn rejects_a_reserved_anchor_from_a_file() {
    assert!(matches!(
        problem_of(&main_file("anchor: \".git/config\"\n")),
        ElementProblem::InvalidAnchor {
            rule: AnchorRule::Reserved,
            ..
        }
    ));
}

#[test]
fn names_a_yaml_anchor_unlike_the_anchor_field() {
    assert_eq!(
        ElementProblem::Anchor.to_string(),
        "the frontmatter uses a YAML anchor"
    );
}

/// `main` with a summary, an anchor, a stamp, and the given kind.
fn main_with_statement(statement: Option<StatementKind>) -> Element {
    let fields = ElementFields {
        statement,
        type_ref: type_ref("t"),
        name: "Main".to_owned(),
        summary: Some("Sum".to_owned()),
        body: String::new(),
        relations: Relations::new(),
        anchor: Some(Anchor::try_from("docs/api.md".to_owned()).unwrap()),
        verified: Some(Verification {
            commit: CommitId::try_from(COMMIT.to_owned()).unwrap(),
            by: Verifier::Ai,
            content: None,
        }),
    };
    Element::new(element_id("main"), fields).unwrap()
}

#[test]
fn writes_a_statement_after_the_anchor_and_before_the_stamp() {
    let element = main_with_statement(Some(StatementKind::Question));

    let expected = format!(
        "id: \"main\"\n\
         type: \"t\"\n\
         name: \"Main\"\n\
         summary: \"Sum\"\n\
         anchor: \"docs/api.md\"\n\
         statement: \"question\"\n\
         verified: \"{COMMIT}\"\n\
         verified_by: \"ai\"\n"
    );
    assert_eq!(frontmatter_of(&element.to_markdown()), expected);
    assert_eq!(read_main(&element.to_markdown()).unwrap(), element);
}

#[test]
fn writes_no_statement_key_without_a_kind() {
    let element = main_with_statement(None);

    assert!(!element.to_markdown().contains("statement"));
    assert_eq!(read_main(&element.to_markdown()).unwrap().statement(), None);
}

#[test]
fn reads_every_statement_kind_and_keeps_it_through_its_fields() {
    for kind in StatementKind::ALL {
        let text = main_file(&format!("statement: {}\n", kind.as_str()));

        let element = read_main(&text).unwrap();

        assert_eq!(element.statement(), Some(kind));
        assert_eq!(element.into_fields().statement, Some(kind));
    }
}

#[rstest]
#[case::empty("statement:\n")]
#[case::empty_quoted("statement: \"\"\n")]
#[case::tilde("statement: ~\n")]
#[case::null_word("statement: null\n")]
#[case::capitalized("statement: Current\n")]
#[case::upper_case("statement: CURRENT\n")]
#[case::not_built_yet("statement: historic\n")]
#[case::trailing_space("statement: \"current \"\n")]
#[case::leading_space("statement: \" current\"\n")]
#[case::trailing_line_feed("statement: \"current\\n\"\n")]
#[case::block_scalar("statement: |\n  current\n")]
#[case::homoglyph("statement: \"\u{441}urrent\"\n")]
fn rejects_any_other_statement_kind(#[case] extra_lines: &str) {
    assert!(matches!(
        problem_of(&main_file(extra_lines)),
        ElementProblem::UnknownStatementKind { .. }
    ));
}

#[test]
fn shows_an_unknown_statement_kind_escaped_and_shortened() {
    let escaping = problem_of(&main_file("statement: \"a\\e[31m\\nb\"\n")).to_string();
    let long = problem_of(&main_file(&format!(
        "statement: \"{}\"\n",
        "x".repeat(10_000)
    )))
    .to_string();

    assert!(!escaping.chars().any(char::is_control), "{escaping:?}");
    assert!(escaping.contains("\\u{1b}"), "{escaping:?}");
    assert!(long.contains('…'), "{long}");
    assert!(long.len() < 500, "{} bytes", long.len());
}

#[test]
fn the_message_of_an_unknown_statement_kind_names_every_kind() {
    let message = ElementProblem::UnknownStatementKind {
        kind: "x".to_owned(),
    }
    .to_string();

    for kind in StatementKind::ALL {
        assert!(message.contains(kind.as_str()), "{message}");
    }
}

#[test]
fn a_statement_counts_toward_the_frontmatter_limit() {
    let with = |statement, length: usize| {
        Element::new(
            element_id("main"),
            ElementFields {
                statement,
                type_ref: type_ref("t"),
                name: "Main".to_owned(),
                summary: Some("a".repeat(length)),
                body: String::new(),
                relations: Relations::new(),
                anchor: None,
                verified: None,
            },
        )
    };
    let largest = (MAX_FRONTMATTER_BYTES - 200..=MAX_FRONTMATTER_BYTES)
        .rev()
        .find(|length| with(None, *length).is_ok())
        .unwrap();

    assert!(matches!(
        with(Some(StatementKind::Idea), largest)
            .unwrap_err()
            .problem(),
        ElementProblem::FrontmatterTooLarge { .. }
    ));
}

const OBJECT: &str = "abcdef0123456789abcdef0123456789abcdef01";

/// The stamp lines of `main` with `content` after `verified_by`.
fn stamped_file(content: &str) -> String {
    main_file(&format!(
        "verified: \"{COMMIT}\"\nverified_by: \"ai\"\n{content}"
    ))
}

/// The element `main`, stamped, with `content` recorded.
fn main_with_content(content: Option<RecordedContent>) -> Element {
    let mut fields = main_with_anchor_and_stamp(Some("crates/x"), Some(Verifier::Ai))
        .unwrap()
        .into_fields();
    if let Some(stamp) = fields.verified.as_mut() {
        stamp.content = content;
    }
    Element::new(element_id("main"), fields).unwrap()
}

fn recorded(entries: &[(&str, Option<&str>)]) -> RecordedContent {
    entries
        .iter()
        .map(|(path, object)| {
            (
                Anchor::try_from((*path).to_owned()).unwrap(),
                object.map(|object| ObjectId::try_from(object.to_owned()).unwrap()),
            )
        })
        .collect()
}

#[test]
fn writes_recorded_content_after_the_verifier_in_the_order_of_its_paths() {
    let element = main_with_content(Some(recorded(&[
        ("crates/y", None),
        ("crates/x", Some(OBJECT)),
    ])));

    let expected = format!(
        "verified: \"{COMMIT}\"\nverified_by: \"ai\"\nverified_content:\n  \
         - {{path: \"crates/x\", object: \"{OBJECT}\"}}\n  - {{path: \"crates/y\"}}\n"
    );
    assert!(
        frontmatter_of(&element.to_markdown()).contains(&expected),
        "{}",
        element.to_markdown()
    );
}

#[rstest]
#[case::no_content(None)]
#[case::no_path(Some(RecordedContent::new()))]
#[case::a_path_that_held_nothing(Some(recorded(&[("crates/x", None)])))]
#[case::a_path_with_its_object(Some(recorded(&[("crates/x", Some(OBJECT))])))]
fn reads_back_recorded_content_byte_for_byte(#[case] content: Option<RecordedContent>) {
    let element = main_with_content(content.clone());
    let markdown = element.to_markdown();

    let read = read_main(&markdown).unwrap();

    assert_eq!(read.verified().unwrap().content, content);
    assert_eq!(read.to_markdown(), markdown);
}

#[test]
fn keeps_no_content_no_path_and_a_path_that_held_nothing_apart() {
    let files: Vec<String> = [
        None,
        Some(RecordedContent::new()),
        Some(recorded(&[("crates/x", None)])),
        Some(recorded(&[("crates/x", Some(OBJECT))])),
    ]
    .into_iter()
    .map(|content| main_with_content(content).to_markdown())
    .collect();

    let distinct: BTreeSet<&String> = files.iter().collect();
    assert_eq!(distinct.len(), files.len());
    assert!(!files[0].contains("verified_content"));
    assert!(files[1].contains("verified_content: []\n"));
}

#[test]
fn reads_a_stamp_without_recorded_content_as_before() {
    let read = read_main(&stamped_file("")).unwrap();

    assert_eq!(read.verified().unwrap().content, None);
}

#[test]
fn reads_hand_written_recorded_content_in_any_style() {
    let read = read_main(&stamped_file(&format!(
        "verified_content:\n  - path: crates/x\n    object: {OBJECT}\n  - {{\"path\": 'crates/y'}}\n"
    )))
    .unwrap();

    assert_eq!(
        read.verified().unwrap().content,
        Some(recorded(&[("crates/x", Some(OBJECT)), ("crates/y", None)]))
    );
}

#[rstest]
#[case::empty_object("verified_content: [{path: x, object: \"\"}]\n", ElementProblem::InvalidObject { object: "\"\"".to_owned() })]
#[case::object_without_value("verified_content: [{path: x, object: }]\n", ElementProblem::InvalidObject { object: "\"\"".to_owned() })]
#[case::tilde_object("verified_content: [{path: x, object: ~}]\n", ElementProblem::InvalidObject { object: "\"~\"".to_owned() })]
#[case::null_object("verified_content: [{path: x, object: null}]\n", ElementProblem::InvalidObject { object: "\"null\"".to_owned() })]
#[case::uppercase_object(
    "verified_content: [{path: x, object: ABCDEF0123456789ABCDEF0123456789ABCDEF01}]\n",
    ElementProblem::InvalidObject { object: "\"ABCDEF0123456789ABCDEF0123456789ABCDEF01\"".to_owned() }
)]
#[case::thirty_nine_digits(
    "verified_content: [{path: x, object: abcdef0123456789abcdef0123456789abcdef0}]\n",
    ElementProblem::InvalidObject { object: "\"abcdef0123456789abcdef0123456789abcdef0\"".to_owned() }
)]
#[case::forty_one_digits(
    "verified_content: [{path: x, object: abcdef0123456789abcdef0123456789abcdef012}]\n",
    ElementProblem::InvalidObject { object: "\"abcdef0123456789abcdef0123456789abcdef012\"".to_owned() }
)]
#[case::sixty_three_digits(
    "verified_content: [{path: x, object: abcdef0123456789abcdef0123456789abcdef0123456789abcdef012345678}]\n",
    ElementProblem::InvalidObject { object: "\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef012345678\"".to_owned() }
)]
#[case::option_as_object(
    "verified_content: [{path: x, object: \"-abcdef0123456789abcdef0123456789abcdef0\"}]\n",
    ElementProblem::InvalidObject { object: "\"-abcdef0123456789abcdef0123456789abcdef0\"".to_owned() }
)]
#[case::empty_value("verified_content:\n", ElementProblem::RecordedContentNotAList)]
#[case::null_value("verified_content: null\n", ElementProblem::RecordedContentNotAList)]
#[case::mapping_value("verified_content: {}\n", ElementProblem::RecordedContentNotAList)]
#[case::plain_entry("verified_content: [x]\n", ElementProblem::RecordedEntryNotAMapping)]
#[case::list_as_a_path(
    "verified_content: [{path: [x]}]\n",
    ElementProblem::NonScalarRecordedValue { key: "\"path\"".to_owned() }
)]
#[case::mapping_as_an_object(
    "verified_content: [{path: x, object: {a: b}}]\n",
    ElementProblem::NonScalarRecordedValue { key: "\"object\"".to_owned() }
)]
#[case::relation_key_in_an_entry(
    "verified_content: [{path: x, kind: uses}]\n",
    ElementProblem::UnknownRecordedKey { key: "\"kind\"".to_owned() }
)]
#[case::object_twice(
    "verified_content: [{path: x, object: a, \"object\": b}]\n",
    ElementProblem::DuplicateRecordedKey { key: "\"object\"".to_owned() }
)]
#[case::missing_path(
    "verified_content: [{object: abcdef0123456789abcdef0123456789abcdef01}]\n",
    ElementProblem::MissingRecordedPath
)]
#[case::path_with_dot_dot(
    "verified_content: [{path: a/../b}]\n",
    ElementProblem::InvalidRecordedPath { path: "\"a/../b\"".to_owned(), rule: AnchorRule::DotSegment }
)]
#[case::path_with_a_leading_slash(
    "verified_content: [{path: /a}]\n",
    ElementProblem::InvalidRecordedPath { path: "\"/a\"".to_owned(), rule: AnchorRule::LeadingSlash }
)]
#[case::path_in_git(
    "verified_content: [{path: a/.git/b}]\n",
    ElementProblem::InvalidRecordedPath { path: "\"a/.git/b\"".to_owned(), rule: AnchorRule::Reserved }
)]
#[case::path_in_abyme(
    "verified_content: [{path: abyme/elements}]\n",
    ElementProblem::InvalidRecordedPath { path: "\"abyme/elements\"".to_owned(), rule: AnchorRule::Reserved }
)]
#[case::path_with_a_zero_width_space(
    "verified_content: [{path: \"a\\u200Bb\"}]\n",
    ElementProblem::InvalidRecordedPath { path: "\"a\\u{200b}b\"".to_owned(), rule: AnchorRule::Character(0x200B) }
)]
#[case::path_quoted_and_plain(
    "verified_content: [{path: \"a\"}, {path: a}]\n",
    ElementProblem::DuplicateRecordedPath { path: "\"a\"".to_owned() }
)]
#[case::anchor_on_the_list("verified_content: &a []\n", ElementProblem::Anchor)]
#[case::tag_on_the_list("verified_content: !!seq []\n", ElementProblem::Tag)]
#[case::alias_as_the_list("verified_content: *a\n", ElementProblem::Alias)]
#[case::anchor_on_an_entry("verified_content:\n  - &a {path: x}\n", ElementProblem::Anchor)]
#[case::tag_on_an_entry("verified_content:\n  - !!map {path: x}\n", ElementProblem::Tag)]
#[case::alias_as_an_entry("verified_content:\n  - *a\n", ElementProblem::Alias)]
#[case::tag_on_a_path("verified_content: [{path: !!str x}]\n", ElementProblem::Tag)]
fn rejects_recorded_content_beyond_its_shape(
    #[case] content: &str,
    #[case] problem: ElementProblem,
) {
    assert_eq!(problem_of(&stamped_file(content)), problem);
}

#[rstest]
#[case::without_a_stamp("verified_content: []\n", ElementProblem::ContentWithoutStamp)]
#[case::with_a_commit_alone(
    &format!("verified: \"{COMMIT}\"\nverified_content: []\n"),
    ElementProblem::IncompleteStamp { missing: "verified_by" }
)]
#[case::with_a_verifier_alone(
    "verified_by: \"ai\"\nverified_content: []\n",
    ElementProblem::IncompleteStamp { missing: "verified" }
)]
fn rejects_recorded_content_without_a_whole_stamp(
    #[case] lines: &str,
    #[case] problem: ElementProblem,
) {
    assert_eq!(problem_of(&main_file(lines)), problem);
}

#[test]
fn shows_a_rejected_key_and_object_of_recorded_content_shortened_and_escaped() {
    let long_key = "k".repeat(10_000);
    let key_problem = problem_of(&stamped_file(&format!(
        "verified_content: [{{path: x, \"{long_key}\\n\\u001b\": y}}]\n"
    )));
    let object_problem = problem_of(&stamped_file(&format!(
        "verified_content: [{{path: x, object: \"{}\\u001b[31m\"}}]\n",
        "a".repeat(10_000)
    )));

    for problem in [key_problem, object_problem] {
        let message = problem.to_string();
        assert!(message.len() < 1_000, "{}", message.len());
        assert!(!message.chars().any(char::is_control), "{message:?}");
    }
}

#[test]
fn refuses_recorded_content_that_takes_the_frontmatter_over_the_limit() {
    let entries = (0..2_000)
        .map(|index| (format!("crates/p{index:05}"), Some(OBJECT)))
        .collect::<Vec<_>>();
    let content: RecordedContent = entries
        .iter()
        .map(|(path, object)| {
            (
                Anchor::try_from(path.clone()).unwrap(),
                object.map(|object| ObjectId::try_from(object.to_owned()).unwrap()),
            )
        })
        .collect();
    let mut fields = main_with_anchor_and_stamp(Some("crates/x"), Some(Verifier::Ai))
        .unwrap()
        .into_fields();
    if let Some(stamp) = fields.verified.as_mut() {
        stamp.content = Some(content);
    }
    let lines = entries
        .iter()
        .map(|(path, _)| format!("  - {{path: \"{path}\", object: \"{OBJECT}\"}}\n"))
        .collect::<Vec<_>>()
        .concat();

    let built = Element::new(element_id("main"), fields);
    let read = problem_of(&stamped_file(&format!("verified_content:\n{lines}")));

    assert!(matches!(
        built.unwrap_err().problem(),
        ElementProblem::FrontmatterTooLarge { .. }
    ));
    assert!(matches!(read, ElementProblem::FrontmatterTooLarge { .. }));
}

#[test]
fn refuses_a_bidi_character_written_as_an_escape_in_a_recorded_path_while_parsing() {
    let problem = problem_of(&stamped_file("verified_content: [{path: \"a\\u202Eb\"}]\n"));

    assert!(
        matches!(problem, ElementProblem::Yaml { .. }),
        "{problem:?}"
    );
}

#[rstest]
#[case::bidi_character('\u{202E}')]
#[case::zero_width_space('\u{200B}')]
fn rejects_a_hidden_character_written_raw_in_a_recorded_path(#[case] character: char) {
    let problem = problem_of(&stamped_file(&format!(
        "verified_content: [{{path: \"a{character}b\"}}]\n"
    )));

    assert_eq!(problem, raw_character_problem(character));
}

#[test]
fn refuses_a_control_character_written_raw_in_a_recorded_path_while_parsing() {
    let problem = problem_of(&stamped_file("verified_content: [{path: \"a\u{1B}b\"}]\n"));

    assert!(
        matches!(problem, ElementProblem::Yaml { .. }),
        "{problem:?}"
    );
}

/// What reading a recorded path with `character` written raw between `a`
/// and `b` reports.
fn raw_character_problem(character: char) -> ElementProblem {
    ElementProblem::InvalidRecordedPath {
        path: shown(&format!("a{character}b"), 200),
        rule: AnchorRule::Character(u32::from(character)),
    }
}
