//! Tests of attaching and detaching notes.

use super::*;

/// A request that adds the note `ask` with a marked label and body, and
/// places it on the board `container-level`, unpinned.
fn a_note_beside_the_element() -> Value {
    json!({"operations": [
        {"op": "create_element", "key": "n", "type_ref": "note", "name": "Ask",
         "body": "SECRET-BODY"},
        {"op": "place_element", "element": {"new": "n"}, "board": {"existing": "container-level"}},
        {"op": "create_relation", "from": {"new": "n"}, "kind": "see_also",
         "to": {"existing": "payment-service"}, "label": "SECRET-LABEL"}
    ]})
}

fn board_text(directory: &Path) -> io::Result<String> {
    fs::read_to_string(directory.join("abyme/boards/container-level.board.json"))
}

#[tokio::test]
async fn apply_attaches_and_detaches_a_note_and_deleting_concerns_ends_the_attachment() -> TestResult
{
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    call_ok(&session, "apply", a_note_beside_the_element()).await?;
    let note_file = directory.path().join("abyme/elements/ask.md");
    let attach = json!({"operations": [
        {"op": "attach_note", "note": {"existing": "ask"}, "to": {"existing": "payment-service"},
         "board": {"existing": "container-level"}}
    ]});

    call_ok(&session, "apply", attach.clone()).await?;
    assert!(
        board_text(directory.path())?.contains(r#""attached_to": "payment-service""#),
        "{}",
        board_text(directory.path())?
    );
    assert!(
        fs::read_to_string(&note_file)?.contains(r#"{kind: "concerns", to: "payment-service"}"#)
    );
    let read = call(
        &session,
        "read_board",
        json!({"board": "container-level", "coordinates": true}),
    )
    .await??;
    assert!(
        text(&read).contains(" on payment-service"),
        "{}",
        text(&read)
    );

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "detach_note", "note": {"existing": "ask"}, "board": {"existing": "container-level"}}
        ]}),
    )
    .await?;
    assert!(!board_text(directory.path())?.contains("attached_to"));
    assert!(
        fs::read_to_string(&note_file)?.contains(r#"{kind: "concerns", to: "payment-service"}"#)
    );

    call_ok(&session, "apply", attach).await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "delete_relation", "from": {"existing": "ask"}, "kind": "concerns",
             "to": {"existing": "payment-service"}}
        ]}),
    )
    .await?;
    assert!(!board_text(directory.path())?.contains("attached_to"));

    closed(session).await
}

#[tokio::test]
async fn apply_refuses_a_wrong_attachment_naming_only_ids_and_changes_no_file() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    call_ok(&session, "apply", a_note_beside_the_element()).await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "move_element", "element": {"existing": "ask"}, "board": {"existing": "container-level"},
             "x": 500, "y": 500, "pinned": true}
        ]}),
    )
    .await?;
    let before = files(directory.path())?;
    let operation = |note: &str, to: &str| {
        json!({"operations": [
            {"op": "attach_note", "note": {"existing": note}, "to": {"existing": to},
             "board": {"existing": "container-level"}}
        ]})
    };
    let refused = [
        (
            operation("payment-service", "ask"),
            "operation 1: element payment-service is no note",
        ),
        (
            operation("ask", "ask"),
            "operation 1: element ask cannot be attached to itself",
        ),
        (
            operation("ask", "payment-service"),
            "operation 1: note ask is pinned on board container-level: a person placed it there. \
             Attach it only after the person has said so, with pinned: true",
        ),
        (
            json!({"operations": [
                {"op": "detach_note", "note": {"existing": "ask"}, "board": {"existing": "container-level"}}
            ]}),
            "operation 1: note ask hangs on nothing on board container-level",
        ),
    ];

    for (arguments, expected) in refused {
        let result = call_refused(&session, "apply", arguments.clone()).await?;

        assert_eq!(result, expected, "{arguments}");
        assert!(!result.contains("SECRET"), "{result:?}");
    }
    for rest in [
        json!({"pinned": false}),
        json!({"x": 1}),
        json!({"label": "why"}),
    ] {
        let mut arguments = operation("ask", "payment-service");
        let attach = arguments
            .pointer_mut("/operations/0")
            .and_then(Value::as_object_mut)
            .ok_or("no operation")?;
        attach.extend(rest.as_object().cloned().unwrap_or_default());

        let result = call_refused(&session, "apply", arguments.clone()).await?;

        assert!(
            result.starts_with("invalid arguments: "),
            "{arguments}: {result:?}"
        );
    }
    assert_eq!(files(directory.path())?, before);

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "attach_note", "note": {"existing": "ask"}, "to": {"existing": "payment-service"},
             "board": {"existing": "container-level"}, "pinned": true}
        ]}),
    )
    .await?;
    assert!(
        board_text(directory.path())?
            .contains(r#""pinned": true, "attached_to": "payment-service""#),
        "{}",
        board_text(directory.path())?
    );

    closed(session).await
}
