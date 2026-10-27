//! Tests of the rules an anchor follows.

use rstest::rstest;

use super::*;
use crate::untrusted::FORMAT_AND_SEPARATORS;

fn rule_of(anchor: &str) -> Option<AnchorRule> {
    match Anchor::try_from(anchor.to_owned()) {
        Ok(_) => None,
        Err(ElementProblem::InvalidAnchor { rule, .. }) => Some(rule),
        Err(other) => panic!("unexpected problem {other:?}"),
    }
}

#[rstest]
#[case::file("crates/payment-service/src/lib.rs")]
#[case::folder("crates/payment-service")]
#[case::one_segment("README.md")]
#[case::deeper_abyme("docs/abyme/x.md")]
#[case::abyme_inside("x/abyme/y")]
#[case::gitignore(".gitignore")]
#[case::github(".github/workflows/ci.yml")]
#[case::ends_in_git("x.git")]
#[case::dots_in_a_name("a..b/c.d")]
#[case::spaces_and_yaml_characters(" a\"b#c 'd&e*f!g%h@i`j ")]
#[case::non_ascii("docs/Überblick.md")]
#[case::leading_dash("-x")]
#[case::glob_characters("a/*/[b]?")]
fn accepts_paths_in_the_project(#[case] anchor: &str) {
    assert_eq!(rule_of(anchor), None);
    assert_eq!(
        Anchor::try_from(anchor.to_owned()).unwrap().as_str(),
        anchor
    );
}

#[rstest]
#[case::empty("", AnchorRule::Empty)]
#[case::absolute("/etc/passwd", AnchorRule::LeadingSlash)]
#[case::file_uri("file:///etc", AnchorRule::Colon)]
#[case::trailing_slash("crates/", AnchorRule::TrailingSlash)]
#[case::empty_segment("a//b", AnchorRule::EmptySegment)]
#[case::dot_segment("a/./b", AnchorRule::DotSegment)]
#[case::dot_dot_segment("a/../b", AnchorRule::DotSegment)]
#[case::parent("../x", AnchorRule::DotSegment)]
#[case::backslash("a\\b", AnchorRule::Backslash)]
#[case::drive("C:/Windows", AnchorRule::Colon)]
#[case::drive_relative("C:x", AnchorRule::Colon)]
#[case::drive_alone("c:", AnchorRule::Colon)]
#[case::pathspec_top(":/x", AnchorRule::Colon)]
#[case::pathspec_exclude(":!x", AnchorRule::Colon)]
#[case::pathspec_magic(":(top)x", AnchorRule::Colon)]
#[case::stream("a/b:c", AnchorRule::Colon)]
#[case::abyme("abyme", AnchorRule::Reserved)]
#[case::abyme_folder("abyme/elements/x.md", AnchorRule::Reserved)]
#[case::abyme_upper("ABYME/x", AnchorRule::Reserved)]
#[case::abyme_mixed("Abyme/x", AnchorRule::Reserved)]
#[case::local_dir(".abyme/lock", AnchorRule::Reserved)]
#[case::git_dir(".git/config", AnchorRule::Reserved)]
#[case::nested_git("x/.GIT/y", AnchorRule::Reserved)]
#[case::git_segment_last("x/.Git", AnchorRule::Reserved)]
fn rejects_what_is_no_path_in_the_project(#[case] anchor: &str, #[case] rule: AnchorRule) {
    assert_eq!(rule_of(anchor), Some(rule));
}

#[rstest]
#[case::null('\u{0}')]
#[case::escape('\u{1b}')]
#[case::line_feed('\n')]
#[case::tab('\t')]
#[case::next_line('\u{85}')]
#[case::delete('\u{7f}')]
#[case::right_to_left_override('\u{202e}')]
#[case::byte_order_mark('\u{feff}')]
#[case::zero_width_space('\u{200b}')]
#[case::zero_width_joiner('\u{200d}')]
#[case::soft_hyphen('\u{ad}')]
#[case::word_joiner('\u{2060}')]
#[case::tag('\u{e0041}')]
#[case::noncharacter('\u{fffe}')]
fn rejects_characters_a_reader_cannot_see(#[case] character: char) {
    let anchor = format!("crates/pay{character}ment");

    assert_eq!(
        rule_of(&anchor),
        Some(AnchorRule::Character(u32::from(character)))
    );
}

#[test]
fn rejects_the_first_and_last_character_of_every_hidden_range() {
    for range in &FORMAT_AND_SEPARATORS {
        for character in [*range.start(), *range.end()] {
            assert_eq!(
                rule_of(&format!("a{character}b")),
                Some(AnchorRule::Character(u32::from(character))),
                "U+{:04X}",
                u32::from(character)
            );
        }
    }
}

#[test]
fn rejects_a_git_folder_hidden_by_a_zero_width_character() {
    assert_eq!(
        rule_of(".g\u{200c}it/x"),
        Some(AnchorRule::Character(0x200c))
    );
}

#[test]
fn accepts_up_to_the_byte_limits() {
    let mut anchor = vec!["a".repeat(100); 40].join("/");
    anchor.push('/');
    anchor.push_str(&"b".repeat(MAX_ANCHOR_BYTES - anchor.len()));

    assert_eq!(anchor.len(), MAX_ANCHOR_BYTES);
    assert_eq!(rule_of(&anchor), None);
    assert_eq!(rule_of(&format!("{anchor}c")), Some(AnchorRule::TooLong));
}

#[rstest]
#[case::ascii("a".repeat(MAX_SEGMENT_BYTES + 1))]
#[case::multi_byte("é".repeat(128))]
fn rejects_a_segment_over_the_byte_limit(#[case] segment: String) {
    assert_eq!(
        rule_of(&format!("x/{segment}")),
        Some(AnchorRule::SegmentTooLong)
    );
}

#[test]
fn accepts_a_segment_at_the_byte_limit_in_multi_byte_characters() {
    let segment = format!("{}a", "é".repeat(127));

    assert_eq!(segment.len(), MAX_SEGMENT_BYTES);
    assert_eq!(rule_of(&format!("x/{segment}")), None);
}

#[test]
fn shows_a_rejected_anchor_escaped_and_shortened() {
    let anchor = format!("\u{1b}[31m\n\u{202e}{}", "x".repeat(64 * 1024));

    let message = Anchor::try_from(anchor).unwrap_err().to_string();

    assert!(!message.chars().any(char::is_control), "{message:?}");
    assert!(!message.contains('\u{202e}'), "{message:?}");
    assert!(message.len() < 1024, "{}", message.len());
}

#[rstest]
#[case::the_path_itself("src", b"src", true)]
#[case::a_file_below("src", b"src/lib.rs", true)]
#[case::a_folder_below("crates/x", b"crates/x/src/", true)]
#[case::a_sibling_with_the_same_start("src", b"srcx/a", false)]
#[case::a_parent("crates/x", b"crates", false)]
#[case::another_case("src", b"Src/lib.rs", false)]
#[case::bytes_that_are_no_text("src", b"src/\xff", true)]
fn covers_the_path_and_everything_below_it(
    #[case] anchor: &str,
    #[case] path: &[u8],
    #[case] covered: bool,
) {
    let anchor = Anchor::try_from(anchor.to_owned()).unwrap();

    assert_eq!(anchor.covers(path), covered);
}

#[rstest]
#[case::the_directory_itself("sub", b"sub", true)]
#[case::a_path_below("sub/x", b"sub", true)]
#[case::two_levels_below("sub/inner/x", b"sub", true)]
#[case::a_sibling_with_the_same_start("sub2", b"sub", false)]
#[case::below_a_sibling_with_the_same_start("sub2/x", b"sub", false)]
#[case::a_shorter_name("su", b"sub", false)]
#[case::a_parent("sub", b"sub/x", false)]
#[case::another_case("Sub/x", b"sub", false)]
#[case::an_empty_directory("sub", b"", false)]
#[case::a_directory_with_a_trailing_slash("sub/x", b"sub/", false)]
fn lies_in_the_directory_or_below_it(
    #[case] anchor: &str,
    #[case] directory: &[u8],
    #[case] lies_in: bool,
) {
    let anchor = Anchor::try_from(anchor.to_owned()).unwrap();

    assert_eq!(anchor.lies_in(directory), lies_in);
}
