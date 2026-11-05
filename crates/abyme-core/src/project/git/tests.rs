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

const TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const BLOB: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

fn object(value: &str) -> ObjectId {
    ObjectId::try_from(value.to_owned()).unwrap()
}

#[test]
fn reads_the_entries_of_ls_tree_without_the_folders_that_enclose_the_project() {
    let output = format!(
        "040000 tree {TREE}\t../\0040000 tree {TREE}\t../../\0040000 tree {TREE}\t.././\0\
         040000 tree {TREE}\t./\0040000 tree {TREE}\tcrates\0100644 blob {BLOB}\tcrates/a b.rs\0\
         100755 blob {BLOB}\trun\0120000 blob {BLOB}\tlink\0160000 commit {TREE}\tvendor/lib\0"
    );

    let entries = tree_entries(output.as_bytes()).unwrap();

    assert_eq!(
        entries,
        BTreeMap::from([
            (b"crates".as_slice(), (ObjectKind::Tree, object(TREE))),
            (
                b"crates/a b.rs".as_slice(),
                (ObjectKind::Blob, object(BLOB))
            ),
            (b"link".as_slice(), (ObjectKind::Blob, object(BLOB))),
            (b"run".as_slice(), (ObjectKind::Blob, object(BLOB))),
            (
                b"vendor/lib".as_slice(),
                (ObjectKind::Gitlink, object(TREE))
            ),
        ])
    );
}

#[rstest]
#[case::path_twice(format!("100644 blob {BLOB}\ta\0100644 blob {TREE}\ta\0"))]
#[case::blob_as_tree(format!("100644 tree {TREE}\ta\0"))]
#[case::tree_as_blob(format!("040000 blob {BLOB}\ta\0"))]
#[case::gitlink_as_blob(format!("160000 blob {BLOB}\ta\0"))]
#[case::unknown_type(format!("100644 tag {BLOB}\ta\0"))]
#[case::uppercase_object(format!("100644 blob {}\ta\0", BLOB.to_uppercase()))]
#[case::thirty_nine_digits(format!("100644 blob {}\ta\0", &BLOB[..39]))]
#[case::empty_path(format!("100644 blob {BLOB}\t\0"))]
#[case::parent_segment(format!("100644 blob {BLOB}\ta/../b\0"))]
#[case::stray_parent(format!("100644 blob {BLOB}\t..\0"))]
#[case::stray_parent_tree(format!("040000 tree {TREE}\t..\0"))]
#[case::dot_segment(format!("100644 blob {BLOB}\t./a\0"))]
#[case::enclosing_folder_as_a_blob(format!("100644 blob {BLOB}\t../\0"))]
#[case::enclosing_folder_with_a_file_mode(format!("100644 tree {TREE}\t../\0"))]
#[case::git_segment(format!("100644 blob {BLOB}\ta/.git/b\0"))]
#[case::empty_segment(format!("100644 blob {BLOB}\ta//b\0"))]
#[case::no_tab(format!("100644 blob {BLOB} a\0"))]
#[case::missing_nul(format!("100644 blob {BLOB}\ta"))]
fn rejects_ls_tree_entries_it_does_not_expect(#[case] output: String) {
    assert_eq!(tree_entries(output.as_bytes()), Err(Unavailable::GitFailed));
}

const OTHER_TREE: &str = "1111111111111111111111111111111111111111";

#[test]
fn confirms_only_candidates_echoed_exactly_as_trees() {
    let tree = object(TREE);
    let other = object(OTHER_TREE);
    let blob = object(BLOB);
    let output = format!("{TREE} tree 59\n{OTHER_TREE} missing\n{BLOB} blob 3\n");

    let confirmed = confirmed(&[&tree, &other, &blob], output.as_bytes());

    assert_eq!(confirmed, Some(BTreeSet::from([tree])));
}

#[rstest]
#[case::commit(format!("{TREE} commit 164\n"))]
#[case::blob(format!("{TREE} blob 3\n"))]
#[case::tag(format!("{TREE} tag 140\n"))]
#[case::ambiguous(format!("{TREE} ambiguous\n"))]
#[case::another_id_echoed(format!("{OTHER_TREE} tree 59\n"))]
#[case::empty_size(format!("{TREE} tree \n"))]
#[case::size_not_decimal(format!("{TREE} tree 5a\n"))]
#[case::trailing_field(format!("{TREE} tree 59 x\n"))]
fn leaves_out_a_candidate_not_confirmed_as_a_tree(#[case] output: String) {
    let tree = object(TREE);

    assert_eq!(
        confirmed(&[&tree], output.as_bytes()),
        Some(BTreeSet::new())
    );
}

#[rstest]
#[case::fewer_lines(format!("{TREE} tree 59\n"))]
#[case::more_lines(format!("{TREE} tree 59\n{OTHER_TREE} tree 59\nx\n"))]
#[case::no_final_line_feed(format!("{TREE} tree 59\n{OTHER_TREE} tree 59"))]
#[case::nothing(String::new())]
fn confirms_nothing_from_batch_check_output_of_another_shape(#[case] output: String) {
    let tree = object(TREE);
    let other = object(OTHER_TREE);

    assert_eq!(confirmed(&[&tree, &other], output.as_bytes()), None);
}

#[test]
fn reads_the_paths_diff_tree_prints() {
    assert_eq!(
        diff_paths(b"a.rs\0src/b c.rs\0"),
        Some(vec![b"a.rs".to_vec(), b"src/b c.rs".to_vec()])
    );
}

#[rstest]
#[case::no_path(b"")]
#[case::empty_path(b"a.rs\0\0")]
#[case::leading_slash(b"/a.rs\0")]
#[case::parent_segment(b"../a.rs\0")]
#[case::dot_segment(b"./a.rs\0")]
#[case::git_segment(b".git/config\0")]
#[case::no_nul(b"a.rs")]
fn reads_no_paths_from_diff_tree_output_that_does_not_fit(#[case] output: &[u8]) {
    assert_eq!(diff_paths(output), None);
}

/// The anchor `path`.
fn anchor(path: &str) -> Anchor {
    Anchor::try_from(path.to_owned()).unwrap()
}

#[test]
fn selects_each_rewritten_anchor_by_what_head_holds_there() {
    let stamp = CommitId::try_from(TREE.to_owned()).unwrap();
    let since = BTreeMap::from([(stamp.clone(), Since::Unknown)]);
    let objects: BTreeMap<Vec<u8>, TreeEntry> = BTreeMap::from([
        (b"tree".to_vec(), (ObjectKind::Tree, object(TREE))),
        (b"same".to_vec(), (ObjectKind::Tree, object(TREE))),
        (b"file".to_vec(), (ObjectKind::Blob, object(BLOB))),
        (b"link".to_vec(), (ObjectKind::Gitlink, object(TREE))),
    ]);
    let longer = ObjectId::try_from("1".repeat(64)).unwrap();
    let cases: Vec<(Anchor, Option<ObjectId>)> = vec![
        (anchor("tree"), Some(object(OTHER_TREE))),
        (anchor("same"), Some(object(TREE))),
        (anchor("file"), Some(longer.clone())),
        (anchor("link"), Some(object(OTHER_TREE))),
        (anchor("gone"), Some(object(OTHER_TREE))),
        (anchor("tree"), None),
        (anchor("tree"), Some(longer)),
    ];
    let contents: Vec<RecordedContent> = cases
        .iter()
        .map(|(anchor, recorded)| RecordedContent::from([(anchor.clone(), recorded.clone())]))
        .collect();
    let elements: Vec<AnchorAndStamp<'_>> = cases
        .iter()
        .zip(&contents)
        .map(|((anchor, _), content)| AnchorAndStamp {
            anchor: Some(anchor),
            stamp: Some(&stamp),
            content: Some(content),
        })
        .collect();
    let members: Vec<&AnchorAndStamp<'_>> = elements.iter().collect();

    let rewritten = rewritten(&members, &since, &objects);

    let other = object(OTHER_TREE);
    let tree = object(TREE);
    let longer = ObjectId::try_from("1".repeat(64)).unwrap();
    assert_eq!(
        rewritten,
        BTreeSet::from([
            Rewritten::Trees {
                anchor: b"tree",
                recorded: &other,
                head: &tree,
            },
            Rewritten::File {
                anchor: b"file",
                recorded: &longer,
            },
        ])
    );
}

#[test]
fn selects_no_anchor_whose_stamp_s_commit_tells_what_changed() {
    let stamp = CommitId::try_from(TREE.to_owned()).unwrap();
    let since = BTreeMap::from([(stamp.clone(), Since::Changed(BTreeSet::new()))]);
    let objects: BTreeMap<Vec<u8>, TreeEntry> =
        BTreeMap::from([(b"file".to_vec(), (ObjectKind::Blob, object(BLOB)))]);
    let file = anchor("file");
    let content = RecordedContent::from([(file.clone(), Some(object(OTHER_TREE)))]);
    let element = AnchorAndStamp {
        anchor: Some(&file),
        stamp: Some(&stamp),
        content: Some(&content),
    };

    assert_eq!(rewritten(&[&element], &since, &objects), BTreeSet::new());
}

#[test]
fn writes_much_input_and_reads_much_output_without_waiting_on_a_full_pipe() {
    let directory = tempfile::tempdir().unwrap();
    abyme_test_support::git(directory.path(), &["init", "-q"]).unwrap();
    let git = Git::find().unwrap();
    let place = Place::project(directory.path());
    let input: String = (0..5000)
        .map(|index| format!("{index:040x}\n"))
        .collect::<Vec<_>>()
        .concat();

    let output = git
        .run_with_input(&place, &["cat-file", "--batch-check"], input.as_bytes())
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout.split(|byte| *byte == b'\n').count(), 5001);
}
