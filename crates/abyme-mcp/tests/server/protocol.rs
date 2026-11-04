//! Tests of the MCP protocol: tools, their arguments, and tool and protocol errors.

use super::*;

#[tokio::test]
async fn applies_operations_and_reads_the_board_back() -> TestResult {
    let (directory, session) = started_project().await?;

    let tools = tokio::time::timeout(TIMEOUT, session.client.list_all_tools()).await??;
    let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "apply",
            "overview",
            "read_board",
            "read_element",
            "search",
            "status",
            "verify"
        ]
    );

    let applied = call_ok(&session, "apply", create_board_with_element()).await?;
    assert_eq!(
        applied,
        "b: board container-level\npay: element payment-service\n"
    );

    let read = call_ok(&session, "read_board", json!({"board": "container-level"})).await?;
    assert_eq!(
        read,
        "Review flags unavailable: the project lies in no Git repository\n\
         Board: container-level · Container Level\n\
         \n\
         payment-service  c4/container  Payment Service  Payments through Stripe\n"
    );
    assert!(
        directory
            .path()
            .join("abyme/elements/payment-service.md")
            .is_file()
    );
    assert!(
        directory
            .path()
            .join("abyme/boards/container-level.board.json")
            .is_file()
    );

    closed(session).await
}

#[tokio::test]
async fn answers_arguments_that_do_not_fit_with_tool_errors_and_keeps_serving() -> TestResult {
    let (directory, session) = started_project().await?;
    let before = files(directory.path())?;
    let not_fitting = [
        ("apply", json!({})),
        ("apply", json!({"operations": [], "extra": 1})),
        (
            "apply",
            json!({"operations": [{"op": "create_board", "key": "k", "title": "t", "unexpected": {}}]}),
        ),
        ("apply", json!({"operations": [["create_board", "k", "t"]]})),
        ("apply", json!({"operations": "create_board"})),
        ("read_board", json!({"board": "../x"})),
        ("read_board", json!({"board": "A"})),
        ("read_board", json!({"board": ""})),
        ("read_board", json!({})),
        ("read_element", json!({"element": "../x"})),
        ("read_element", json!({"element": "a/b"})),
        ("read_element", json!({"element": "X"})),
        ("read_element", json!({"element": ""})),
        ("read_element", json!({"element": "a".repeat(65)})),
        ("read_element", json!({"element": 1})),
        ("read_element", json!({"element": "x", "path": "/etc"})),
        ("read_element", json!({})),
    ];

    for (tool, arguments) in not_fitting {
        let result = call_refused(&session, tool, arguments.clone()).await?;

        assert!(
            result.starts_with("invalid arguments: "),
            "{tool} {arguments}: {result:?}"
        );
    }

    let unknown_board = call_refused(&session, "read_board", json!({"board": "nowhere"})).await?;
    assert_eq!(unknown_board, "no board has the ID nowhere");
    assert_eq!(files(directory.path())?, before);

    call_ok(&session, "apply", create_board_with_element()).await?;

    closed(session).await
}

#[tokio::test]
async fn escapes_and_shortens_what_it_quotes_from_rejected_arguments() -> TestResult {
    let (_directory, session) = started_project().await?;

    let hostile = call(
        &session,
        "apply",
        json!({"operations": [], "\u{1b}[31m\u{202e}x": 1}),
    )
    .await??;
    let long = call(
        &session,
        "apply",
        json!({"operations": [], "x".repeat(10_000): 1}),
    )
    .await??;

    let hostile = text(&hostile);
    assert!(hostile.contains("\\u{1b}[31m\\u{202e}x"), "{hostile}");
    assert!(
        !hostile.contains('\u{1b}') && !hostile.contains('\u{202e}'),
        "{hostile}"
    );
    let long = text(&long);
    assert!(long.contains('…'), "{long}");
    assert!(
        long.chars().count() < 400,
        "{} characters",
        long.chars().count()
    );

    closed(session).await
}

#[tokio::test]
async fn a_rejected_apply_is_a_tool_error_and_changes_no_file() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    let before = files(directory.path())?;

    let rejected = call_refused(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "e", "type_ref": "t", "name": "New"},
            {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "nope"}}
        ]}),
    )
    .await?;

    assert_eq!(rejected, "operation 2: no board has the ID nope");
    assert_eq!(files(directory.path())?, before);

    closed(session).await
}

#[tokio::test]
async fn read_board_takes_coordinates_only_as_an_optional_boolean() -> TestResult {
    let (_directory, session) = started_project().await?;

    let tools = tokio::time::timeout(TIMEOUT, session.client.list_all_tools()).await??;
    let read_board = tools
        .iter()
        .find(|tool| tool.name == "read_board")
        .ok_or("no read_board")?;
    let schema = &read_board.input_schema;
    assert_eq!(
        schema
            .get("properties")
            .and_then(|properties| properties.get("coordinates"))
            .and_then(|coordinates| coordinates.get("type")),
        Some(&json!("boolean")),
        "{schema:?}"
    );
    assert_eq!(
        schema.get("required"),
        Some(&json!(["board"])),
        "{schema:?}"
    );
    assert_eq!(
        schema.get("additionalProperties"),
        Some(&json!(false)),
        "{schema:?}"
    );

    for arguments in [
        json!({"board": "main", "coordinates": "yes"}),
        json!({"board": "main", "coordinates": 1}),
        json!({"board": "main", "coordinates": null}),
        json!({"coordinates": true}),
    ] {
        let result = call_refused(&session, "read_board", arguments.clone()).await?;

        assert!(
            result.starts_with("invalid arguments: "),
            "{arguments}: {result:?}"
        );
    }
    closed(session).await
}

#[tokio::test]
async fn an_unknown_tool_is_a_protocol_error_and_the_server_keeps_serving() -> TestResult {
    let (_directory, session) = started_project().await?;

    let unknown = call(&session, "delete_everything", json!({})).await?;

    match unknown {
        Err(ServiceError::McpError(error)) => assert_eq!(error.code, ErrorCode::INVALID_PARAMS),
        other => panic!("expected a protocol error, got {other:?}"),
    }
    call_ok(&session, "apply", create_board_with_element()).await?;

    closed(session).await
}

#[tokio::test]
async fn applies_two_requests_at_once_without_losing_either() -> TestResult {
    let (_directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    let place = |name: &str| {
        json!({"operations": [
            {"op": "create_element", "key": "e", "type_ref": "t", "name": name},
            {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "container-level"}}
        ]})
    };

    let (left, right) = tokio::join!(
        call_ok(&session, "apply", place("Left")),
        call_ok(&session, "apply", place("Right"))
    );
    left?;
    right?;
    let read = call(&session, "read_board", json!({"board": "container-level"})).await??;
    let outline = text(&read);
    assert!(outline.contains("\nleft  "), "{outline}");
    assert!(outline.contains("\nright  "), "{outline}");
    assert!(outline.contains("\npayment-service  "), "{outline}");

    closed(session).await
}

/// Not a parameter of any tool: the client cannot steer where the server
/// looks for the project.
#[tokio::test]
async fn no_tool_takes_a_path() -> TestResult {
    let (_directory, session) = started_project().await?;

    let tools = tokio::time::timeout(TIMEOUT, session.client.list_all_tools()).await??;

    for tool in &tools {
        let properties: Vec<&String> = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .map(JsonObject::keys)
            .into_iter()
            .flatten()
            .collect();
        let expected: &[&str] = match tool.name.as_ref() {
            "apply" => &["operations"],
            "read_board" => &["board", "coordinates"],
            "read_element" | "verify" => &["element"],
            "search" => &["query", "statement"],
            "overview" | "status" => &[],
            other => return Err(format!("unexpected tool {other}").into()),
        };
        assert_eq!(properties, expected, "{}", tool.name);
        assert_eq!(
            tool.input_schema.get("additionalProperties"),
            Some(&json!(false)),
            "{}",
            tool.name
        );
    }
    closed(session).await
}
