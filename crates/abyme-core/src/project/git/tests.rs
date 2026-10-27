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
            uncommitted: set(&[b"changed in tree", b"deleted", b"conflict", b"new file"]),
            untracked: set(&[b"new file"]),
            repositories: set(&[b"clone"]),
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

const OBJECT: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn reads_the_entries_of_ls_files() {
    let output = format!(
        "100644 {OBJECT} 0\tREADME\0\
         160000 {OBJECT} 0\tvendor/lib\0\
         100755 {OBJECT}{} 2\ta b\tc\0",
        &OBJECT[..24]
    );

    assert_eq!(
        staged(output.as_bytes()),
        Ok(vec![
            (false, b"README".as_slice()),
            (true, b"vendor/lib"),
            (false, b"a b\tc"),
        ])
    );
}

#[rstest]
#[case::without_a_tab(format!("100644 {OBJECT} 0 a"))]
#[case::a_short_mode(format!("10644 {OBJECT} 0\ta"))]
#[case::a_mode_that_is_not_octal(format!("100684 {OBJECT} 0\ta"))]
#[case::a_short_object(format!("100644 {} 0\ta", &OBJECT[..39]))]
#[case::an_object_in_capitals(format!("100644 {} 0\ta", OBJECT.to_uppercase()))]
#[case::an_unknown_stage(format!("100644 {OBJECT} 4\ta"))]
#[case::an_extra_field(format!("100644 {OBJECT} 0 0\ta"))]
#[case::an_empty_path(format!("100644 {OBJECT} 0\t"))]
fn rejects_ls_files_entries_it_does_not_expect(#[case] record: String) {
    assert_eq!(staged_entry(record.as_bytes()), Err(Unavailable::GitFailed));
}

#[rstest]
#[case::a_file(b"a.rs", b"sub/a.rs")]
#[case::a_path_with_spaces_and_dots(b"a b/.x/c..d", b"sub/a b/.x/c..d")]
#[case::a_git_file(b"src/.gitignore", b"sub/src/.gitignore")]
fn puts_a_nested_path_below_its_root(#[case] path: &[u8], #[case] expected: &[u8]) {
    assert_eq!(nested_path(b"sub", path), Ok(expected.to_vec()));
}

#[rstest]
#[case::empty(b"")]
#[case::absolute(b"/etc/passwd")]
#[case::an_empty_segment(b"a//b")]
#[case::a_final_slash(b"a/")]
#[case::the_top(b".")]
#[case::the_parent(b"..")]
#[case::below_the_parent(b"../x")]
#[case::through_the_parent(b"a/../../x")]
#[case::through_the_top(b"./a")]
#[case::gits_own_directory(b".git")]
#[case::inside_gits_own_directory(b"a/.git/config")]
fn refuses_a_nested_path_that_leads_out_of_its_repository(#[case] path: &[u8]) {
    assert_eq!(nested_path(b"sub", path), Err(Unavailable::GitFailed));
}

#[test]
fn distrusts_every_filter_driver_a_scope_but_global_or_system_sets() {
    let output: &[u8] = b"global\0filter.lfs.clean\0\
        system\0filter.sys.process\0\
        local\0filter.Evil.a=b.clean\0\
        worktree\0filter.tree.process\0\
        command\0filter.cmd.required\0\
        file\0filter.unknown.clean\0\
        global\0filter.both.clean\0\
        local\0filter.both.smudge\0\
        local\0filter.nodriver\0";

    assert_eq!(
        untrusted_drivers(output),
        Ok(set(&[b"Evil.a=b", b"tree", b"cmd", b"unknown", b"both"]))
    );
}

#[rstest]
#[case::a_scope_without_a_name(b"local\0".as_slice())]
#[case::a_key_of_another_section(b"local\0core.fsmonitor\0".as_slice())]
#[case::without_a_final_nul(b"local\0filter.x.clean".as_slice())]
fn rejects_a_listing_of_filter_drivers_it_does_not_expect(#[case] output: &[u8]) {
    assert_eq!(untrusted_drivers(output), Err(Unavailable::GitFailed));
}

#[test]
fn turns_off_hooks_and_the_given_filter_drivers_through_the_environment() {
    let drivers = set(&[b"Evil.a=b"]);

    let settings = nested_settings(Path::new("/project/vendor"), &drivers);

    let expected: Vec<(OsString, OsString)> = [
        ("GIT_CEILING_DIRECTORIES", "/project/vendor"),
        ("GIT_CONFIG_COUNT", "4"),
        ("GIT_CONFIG_KEY_0", "core.hooksPath"),
        ("GIT_CONFIG_VALUE_0", "/dev/null"),
        ("GIT_CONFIG_KEY_1", "filter.Evil.a=b.clean"),
        ("GIT_CONFIG_VALUE_1", ""),
        ("GIT_CONFIG_KEY_2", "filter.Evil.a=b.process"),
        ("GIT_CONFIG_VALUE_2", ""),
        ("GIT_CONFIG_KEY_3", "filter.Evil.a=b.required"),
        ("GIT_CONFIG_VALUE_3", "false"),
    ]
    .into_iter()
    .map(|(name, value)| (name.into(), value.into()))
    .collect();
    assert_eq!(settings, expected);
}

#[rstest]
#[case::the_project(Place::project(Path::new("/project")))]
#[case::a_nested_repository(Place {
    directory: PathBuf::from("/project/vendor/lib"),
    root: Some(b"vendor/lib".to_vec()),
    settings: nested_settings(Path::new("/project/vendor"), &BTreeSet::new()),
})]
fn runs_git_without_transports_and_with_its_place_s_settings(#[case] place: Place) {
    let git = Git {
        program: PathBuf::from("/usr/bin/git"),
    };

    let command = git.command(&place, &["status"]);

    let environment: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
    for (name, value) in ENVIRONMENT
        .iter()
        .map(|(name, value)| (OsStr::new(name), OsStr::new(value)))
        .chain(
            place
                .settings
                .iter()
                .map(|(name, value)| (name.as_os_str(), value.as_os_str())),
        )
    {
        assert!(
            environment.contains(&(name, Some(value))),
            "{name:?} in {environment:?}"
        );
    }
    assert!(environment.contains(&(OsStr::new("GIT_ALLOW_PROTOCOL"), Some(OsStr::new("")))));
    assert_eq!(command.get_current_dir(), Some(place.directory.as_path()));
}

#[test]
fn reads_the_root_of_an_untracked_repository_without_its_final_slash() {
    assert_eq!(
        status_record(b"? vendor/clone/"),
        Ok(StatusRecord::Repository(b"vendor/clone"))
    );
}
