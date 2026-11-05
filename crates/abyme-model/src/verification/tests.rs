//! Tests of the commit IDs and verifiers a stamp holds.

use rstest::rstest;

use super::*;

#[rstest]
#[case::sha1("0123456789abcdef0123456789abcdef01234567")]
#[case::sha256("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")]
#[case::all_zero("0000000000000000000000000000000000000000")]
fn accepts_a_full_commit_id(#[case] commit: &str) {
    assert_eq!(
        CommitId::try_from(commit.to_owned()).unwrap().as_str(),
        commit
    );
}

#[rstest]
#[case::head("HEAD")]
#[case::option("--output=x")]
#[case::dash("-x")]
#[case::abbreviated("0123456")]
#[case::uppercase("0123456789ABCDEF0123456789ABCDEF01234567")]
#[case::forty_one("0123456789abcdef0123456789abcdef012345678")]
#[case::sixty_three("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde")]
#[case::fullwidth_digits(
    "０１２３４５６７８９０１２３４５６７８９０１２３４５６７８９０１２３４５６７８９"
)]
#[case::arabic_indic_digits("٠١٢٣٤٥٦٧٨٩٠١٢٣٤٥٦٧٨٩٠١٢٣٤٥٦٧٨٩٠١٢٣٤٥٦٧٨٩")]
#[case::inner_space("0123456789abcdef0123 56789abcdef01234567")]
#[case::inner_line_feed("0123456789abcdef0123\n56789abcdef01234567")]
#[case::suffix("0123456789abcdef0123456789abcdef01234567^{}")]
fn rejects_anything_but_a_full_lowercase_commit_id(#[case] commit: &str) {
    assert!(matches!(
        CommitId::try_from(commit.to_owned()),
        Err(ElementProblem::InvalidCommit { .. })
    ));
}

#[test]
fn reads_both_names_of_verifiers_and_no_other() {
    assert_eq!(Verifier::from_name("ai"), Some(Verifier::Ai));
    assert_eq!(Verifier::from_name("person"), Some(Verifier::Person));
    for name in ["AI", " ai", "ai ", "bot", "Person", ""] {
        assert_eq!(Verifier::from_name(name), None, "{name:?}");
    }
}

#[rstest]
#[case::commit_alone(Some("0123456789abcdef0123456789abcdef01234567"), None, "verified_by")]
#[case::verifier_alone(None, Some("ai"), "verified")]
fn rejects_half_a_stamp(
    #[case] verified: Option<&str>,
    #[case] verified_by: Option<&str>,
    #[case] missing: &'static str,
) {
    assert_eq!(
        stamp(
            verified.map(str::to_owned),
            verified_by.map(str::to_owned),
            None
        ),
        Err(ElementProblem::IncompleteStamp { missing })
    );
}

#[test]
fn shows_a_rejected_commit_and_verifier_escaped() {
    let commit = CommitId::try_from("\u{1b}[31mx".to_owned())
        .unwrap_err()
        .to_string();
    let verifier = stamp(
        Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
        Some("bot\n\u{1b}".to_owned()),
        None,
    )
    .unwrap_err()
    .to_string();

    for message in [commit, verifier] {
        assert!(!message.chars().any(char::is_control), "{message:?}");
    }
}
