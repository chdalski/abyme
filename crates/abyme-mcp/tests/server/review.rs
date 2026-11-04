//! Tests of anchors, stamps, review flags, `verify`, and `status`.

use super::*;

/// A hand-written element file with an anchor and a stamp by a person.
const STAMPED: &str = "---\nid: stamped\ntype: t\nname: Stamped\nanchor: crates/stamped\n\
    verified: \"0123456789abcdef0123456789abcdef01234567\"\nverified_by: person\n---\n";

/// A project with the hand-written element `stamped`.
fn project_with_a_stamped_element() -> TestResult<tempfile::TempDir> {
    let directory = project()?;
    let elements = directory.path().join("abyme/elements");
    fs::create_dir_all(&elements)?;
    fs::write(elements.join("stamped.md"), STAMPED)?;
    Ok(directory)
}

#[tokio::test]
async fn apply_sets_and_changes_anchors_and_a_changed_anchor_removes_the_stamp() -> TestResult {
    let directory = project_with_a_stamped_element()?;
    let session = start(directory.path()).await?;

    // A colon is refused.
    call_refused(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "d", "type_ref": "doc", "name": "Docs Page",
             "anchor": "docs/a \"b\" #c: d"}
        ]}),
    )
    .await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "d", "type_ref": "doc", "name": "Docs Page",
             "anchor": "docs/a \"b\" #c"}
        ]}),
    )
    .await?;
    let read = call(&session, "read_element", json!({"element": "docs-page"})).await??;
    assert!(
        text(&read).starts_with(
            "Review flags unavailable: the project lies in no Git repository\n\
             Element: docs-page · Docs Page\n\
             Type: doc\n\
             Statement kind: none\n\
             Anchor: docs/a \"b\" #c\n\
             Verified: none\n"
        ),
        "{}",
        text(&read)
    );

    let read = call(&session, "read_element", json!({"element": "stamped"})).await??;
    assert!(
        text(&read).contains(
            "\nAnchor: crates/stamped\n\
             Verified: 0123456789abcdef0123456789abcdef01234567 by person\n"
        ),
        "{}",
        text(&read)
    );

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_element", "element": {"existing": "stamped"}, "anchor": "crates/other"}
        ]}),
    )
    .await?;
    let read = call(&session, "read_element", json!({"element": "stamped"})).await??;
    assert!(
        text(&read).contains("\nAnchor: crates/other\nVerified: none\n"),
        "{}",
        text(&read)
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("abyme/elements/stamped.md"))?,
        "---\n\
         id: \"stamped\"\n\
         type: \"t\"\n\
         name: \"Stamped\"\n\
         anchor: \"crates/other\"\n\
         ---\n"
    );

    closed(session).await
}

#[tokio::test]
async fn apply_removes_the_stamp_when_the_content_of_an_element_changes() -> TestResult {
    let directory = project_with_a_stamped_element()?;
    let session = start(directory.path()).await?;

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_element", "element": {"existing": "stamped"}, "name": "Renamed"}
        ]}),
    )
    .await?;

    let read = call(&session, "read_element", json!({"element": "stamped"})).await??;
    assert!(
        text(&read).contains("\nAnchor: crates/stamped\nVerified: none\n"),
        "{}",
        text(&read)
    );
    let file = fs::read_to_string(directory.path().join("abyme/elements/stamped.md"))?;
    assert!(!file.contains("verified"), "{file}");

    closed(session).await
}

#[tokio::test]
async fn read_element_shows_the_review_flags_git_reports() -> TestResult {
    let directory = repository_with_a_changed_anchor()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let read = call_ok(&session, "read_element", json!({"element": "x"})).await?;

    assert!(read.starts_with("Element: x · X\n"), "{read}");
    assert!(read.contains(CHANGED_ANCHOR_REVIEW), "{read}");
    closed(session).await
}

#[tokio::test]
async fn read_element_shows_each_kind_of_review_flag() -> TestResult {
    let directory = project()?;
    let root = directory.path();
    git(root, &["init", "-q"])?;
    for file in ["crates/c/lib.rs", "crates/u/lib.rs"] {
        fs::create_dir_all(root.join(file).parent().ok_or("no parent")?)?;
        fs::write(root.join(file), "fn a() {}\n")?;
    }
    let stamp = commit_all(root)?;
    let unknown = "0123456789abcdef0123456789abcdef01234567";
    write_element(root, "committed", Some("crates/c"), Some(&stamp))?;
    write_element(root, "untracked", Some("crates/u"), Some(&stamp))?;
    write_element(root, "unknown", Some("crates/c"), Some(unknown))?;
    write_element(root, "missing", Some("crates/gone"), Some(&stamp))?;
    write_element(root, "unstamped", Some("crates/c"), None)?;
    write_element(root, "native", None, None)?;
    fs::write(root.join("crates/c/lib.rs"), "fn a() { b() }\n")?;
    commit_all(root)?;
    fs::write(root.join("crates/u/new.rs"), "fn n() {}\n")?;
    let config = tempfile::tempdir()?;
    let session = start_with(root, &[("XDG_CONFIG_HOME", config.path().as_os_str())]).await?;
    let cases = [
        (
            "committed",
            "Review: task\n  changed since the stamp: crates/c/lib.rs\n",
        ),
        (
            "untracked",
            "Review: task\n  changed since the stamp: crates/u/new.rs\n",
        ),
        (
            "unknown",
            "Review: task\n  the stamp's commit is unknown to the repository or no ancestor of HEAD\n",
        ),
        (
            "missing",
            "Review: task\n  no file that Git knows lies under the anchor\n",
        ),
        ("unstamped", "Review: task\n  never verified\n"),
        ("native", "Review: none\n"),
    ];

    for (id, review) in cases {
        let read = call_ok(&session, "read_element", json!({"element": id})).await?;

        assert!(
            read.contains(&format!("\n{review}Boards: none\n")),
            "{id}: {read}"
        );
    }
    closed(session).await
}

#[tokio::test]
async fn read_element_shows_flags_passed_on_along_relations() -> TestResult {
    let directory = project()?;
    let root = directory.path();
    git(root, &["init", "-q"])?;
    fs::create_dir_all(root.join("crates/db"))?;
    fs::write(root.join("crates/db/schema.sql"), "create table a;\n")?;
    let stamp = commit_all(root)?;
    write_element(root, "orders-db", Some("crates/db"), Some(&stamp))?;
    for (id, kind) in [("pay", "depends_on"), ("note", "uses")] {
        fs::write(
            root.join(format!("abyme/elements/{id}.md")),
            format!(
                "---\nid: {id}\ntype: t\nname: N\nverified: \"{stamp}\"\nverified_by: ai\n\
                 relations:\n  - {{kind: {kind}, to: orders-db, label: \"secret label\"}}\n---\n"
            ),
        )?;
    }
    fs::write(root.join("crates/db/schema.sql"), "create table b;\n")?;
    let config = tempfile::tempdir()?;
    let session = start_with(root, &[("XDG_CONFIG_HOME", config.path().as_os_str())]).await?;
    let cases = [
        (
            "pay",
            "Review: task\n  depends_on orders-db: what its anchor stands for may have changed since this element's stamp\n",
        ),
        (
            "note",
            "Review: hint\n  uses orders-db: what its anchor stands for may have changed since this element's stamp\n",
        ),
    ];

    for (id, review) in cases {
        let read = call(&session, "read_element", json!({"element": id})).await??;

        let reply = text(&read);
        assert!(
            reply.contains(&format!("\n{review}Boards: none\n")),
            "{id}: {reply}"
        );
        let line = reply
            .lines()
            .find(|line| line.contains("orders-db: what"))
            .ok_or("no passed-on flag")?;
        assert!(!line.contains("schema.sql"), "{line}");
        assert!(!line.contains("secret"), "{line}");
    }
    closed(session).await
}

#[tokio::test]
async fn read_board_marks_the_elements_to_review() -> TestResult {
    let directory = repository_with_marked_elements()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let read = call(&session, "read_board", json!({"board": "b"})).await??;
    let detail = call(&session, "read_element", json!({"element": "changed"})).await??;

    assert_eq!(
        text(&read),
        "Board: b · B\n\
         \n\
         calm     t  N\n\
         changed  t  N    ⚠ task\n\
         clean    t  N\n\
         ghost    ⚠ missing\n\
         hinted   t  N    ⚠ hint\n\
         \n\
         hinted → changed  uses\n"
    );
    assert!(
        text(&detail).contains("leak-canary.txt"),
        "{}",
        text(&detail)
    );
    closed(session).await
}

#[tokio::test]
async fn verify_stamps_a_checked_element_and_clears_its_flag() -> TestResult {
    let directory = repository_with_a_changed_anchor()?;
    let head = commit_all(directory.path())?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let verified = call_ok(&session, "verify", json!({"element": "x"})).await?;
    let read = call(&session, "read_element", json!({"element": "x"})).await??;

    assert_eq!(verified, format!("Verified x against {head} by ai."));
    assert!(
        text(&read).contains(&format!("\nVerified: {head} by ai\nReview: none\n")),
        "{}",
        text(&read)
    );
    closed(session).await
}

#[tokio::test]
async fn verify_refuses_and_writes_nothing() -> TestResult {
    let directory = repository_with_a_changed_anchor()?;
    let empty_path = tempfile::tempdir()?;
    let config = tempfile::tempdir()?;
    let abyme = directory.path().join("abyme");
    let before = files(&abyme)?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;
    let cases = [
        (
            json!({"element": "x"}),
            "element x cannot be verified: files under its anchor differ from HEAD or are untracked; they must be committed first",
        ),
        (
            json!({"element": "nowhere"}),
            "no element has the ID nowhere",
        ),
    ];

    for (arguments, expected) in cases {
        let refused = call_refused(&session, "verify", arguments).await?;

        assert_eq!(refused, expected);
    }
    for arguments in [
        json!({"element": "x", "commit": "0123456789abcdef0123456789abcdef01234567", "by": "person"}),
        json!({"element": "../x"}),
    ] {
        let refused = call_refused(&session, "verify", arguments).await?;

        assert!(refused.starts_with("invalid arguments: "), "{refused:?}");
    }
    assert_eq!(files(&abyme)?, before);
    closed(session).await?;

    let session = start_with(directory.path(), &[("PATH", empty_path.path().as_os_str())]).await?;
    let refused = call(&session, "verify", json!({"element": "x"})).await??;
    assert_eq!(
        text(&refused),
        "element x cannot be verified: git could not be run"
    );
    assert_eq!(files(&abyme)?, before);
    closed(session).await
}

#[tokio::test]
async fn verify_refuses_in_a_repository_without_a_commit() -> TestResult {
    let directory = project()?;
    git(directory.path(), &["init", "-q"])?;
    write_element(directory.path(), "x", None, None)?;
    let abyme = directory.path().join("abyme");
    let before = files(&abyme)?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let refused = call_refused(&session, "verify", json!({"element": "x"})).await?;

    assert_eq!(
        refused,
        "element x cannot be verified: the repository has no commit yet"
    );
    assert_eq!(files(&abyme)?, before);
    closed(session).await
}

#[tokio::test]
async fn status_lists_the_elements_to_review_without_file_names() -> TestResult {
    let directory = repository_with_marked_elements()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let status = call_ok(&session, "status", json!({})).await?;

    assert_eq!(
        status,
        "Elements to review: 1 with a task, 1 with only a hint.\n\
         changed  t  N    ⚠ task\n\
         hinted   t  N    ⚠ hint\n"
    );
    closed(session).await
}

#[tokio::test]
async fn status_names_why_review_flags_are_unavailable_and_takes_no_argument() -> TestResult {
    let (_directory, session) = started_project().await?;

    let unavailable = call_refused(&session, "status", json!({})).await?;
    let with_argument = call_refused(&session, "status", json!({"x": 1})).await?;

    assert_eq!(
        unavailable,
        "Review flags unavailable: the project lies in no Git repository"
    );
    assert!(
        with_argument.starts_with("invalid arguments: "),
        "{with_argument:?}"
    );
    closed(session).await
}

#[tokio::test]
async fn read_element_names_why_review_flags_are_unavailable() -> TestResult {
    let without_git = repository_with_a_changed_anchor()?;
    let empty_path = tempfile::tempdir()?;
    let broken = repository_with_a_changed_anchor()?;
    fs::write(broken.path().join(".git/index"), "not an index")?;
    let cases = [
        (
            without_git.path(),
            Some(empty_path.path()),
            "git could not be run",
        ),
        (broken.path(), None, "git failed"),
    ];

    for (directory, path, reason) in cases {
        let variables: Vec<(&str, &OsStr)> = path
            .map(|path| ("PATH", path.as_os_str()))
            .into_iter()
            .collect();
        let session = start_with(directory, &variables).await?;

        let read = call(&session, "read_element", json!({"element": "x"})).await??;

        let reply = text(&read);
        assert!(
            reply.starts_with(&format!(
                "Review flags unavailable: {reason}\nElement: x · X\n"
            )),
            "{reply}"
        );
        assert!(!reply.contains("Review:"), "{reply}");
        assert!(!reply.contains("fatal"), "{reply}");
        assert!(!reply.contains("error:"), "{reply}");
        assert!(!reply.contains(&directory.display().to_string()), "{reply}");
        closed(session).await?;
    }
    Ok(())
}

#[tokio::test]
async fn apply_refuses_to_set_a_stamp_and_changes_no_file() -> TestResult {
    let directory = project_with_a_stamped_element()?;
    let session = start(directory.path()).await?;
    let before = files(directory.path())?;

    // With a name to change, only the stamp field can make the request fail.
    for (field, value) in [
        ("verified", "0123456789abcdef0123456789abcdef01234567"),
        ("verified_by", "person"),
    ] {
        let mut update =
            json!({"op": "update_element", "element": {"existing": "stamped"}, "name": "X"});
        update[field] = json!(value);
        let refused = call_refused(&session, "apply", json!({"operations": [update]})).await?;

        assert!(refused.contains(field), "{refused}");
        assert_eq!(files(directory.path())?, before);
    }

    closed(session).await
}
