//! Tests of reading what Git prints.

use rstest::rstest;

use super::*;

fn set(values: &[&[u8]]) -> BTreeSet<Vec<u8>> {
    values.iter().map(|value| value.to_vec()).collect()
}

#[rstest]
#[case::nothing(b"", vec![])]
#[case::one(b"a\0", vec![b"a".as_slice()])]
#[case::names_with_line_breaks_and_tabs(b"a\nb\0c\td\0", vec![b"a\nb".as_slice(), b"c\td"])]
#[case::bytes_that_are_no_text(b"\xff\0", vec![b"\xff".as_slice()])]
fn reads_nul_terminated_records(#[case] output: &[u8], #[case] expected: Vec<&[u8]>) {
    assert_eq!(records(output), Ok(expected));
}

#[test]
fn rejects_output_that_does_not_end_with_nul() {
    assert_eq!(records(b"a\0b"), Err(Unavailable::GitFailed));
}

#[test]
fn reads_the_paths_of_git_status() {
    let output: &[u8] = b"1 .M N... 100644 100644 100644 0123 0123 p/changed in tree\0\
        1 M. N... 100644 100644 100644 0123 4567 p/staged only\0\
        1 .D N... 100644 100644 000000 0123 0123 p/deleted\0\
        u UU N... 100644 100644 100644 100644 0123 4567 89ab p/conflict\0\
        ? p/new file\0\
        ? p/clone/\0";

    let status = status_paths(output, b"p/").unwrap();

    assert_eq!(
        status,
        Status {
            uncommitted: set(&[
                b"changed in tree",
                b"deleted",
                b"conflict",
                b"new file",
                b"clone/",
            ]),
            untracked: set(&[b"new file", b"clone/"]),
        }
    );
}

#[rstest]
#[case::renamed(b"2 R. N... 100644 100644 100644 0123 0123 R100 p/b\0p/a\0".as_slice())]
#[case::ignored(b"! p/target\0".as_slice())]
#[case::header(b"# branch.oid 0123\0".as_slice())]
#[case::too_few_fields(b"1 .M N... p/a\0".as_slice())]
#[case::outside_the_project(b"? q/a\0".as_slice())]
#[case::the_project_itself(b"? p/\0".as_slice())]
#[case::without_a_final_nul(b"? p/a".as_slice())]
fn rejects_status_records_it_does_not_expect(#[case] output: &[u8]) {
    assert_eq!(status_paths(output, b"p/"), Err(Unavailable::GitFailed));
}

#[test]
fn keeps_paths_at_the_top_of_the_work_tree() {
    let status = status_paths(b"? a\0", b"").unwrap();

    assert_eq!(status.untracked, set(&[b"a"]));
}

#[test]
fn reads_a_path_with_spaces_after_the_fixed_fields() {
    let record = b"1 .M N... 100644 100644 100644 0123 0123 a b c";

    assert_eq!(
        status_record(record),
        Ok(StatusRecord::Changed {
            path: b"a b c",
            in_working_tree: true,
        })
    );
}
