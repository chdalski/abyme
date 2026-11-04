//! Tests of elements: relations, updates, deletion, the detail, and statement kinds.

use super::*;

#[tokio::test]
async fn the_read_tools_show_each_element_s_kind_from_its_home_board_s_zone() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "ctx", "title": "Context",
             "zone": {"mode": "mirrored", "statement": "current"}},
            {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
            {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
            {"op": "create_board", "key": "z", "title": "Zeta",
             "zone": {"mode": "free", "statement": "idea"}},
            {"op": "create_element", "key": "spark", "type_ref": "note", "name": "Spark"},
            {"op": "place_element", "element": {"new": "spark"}, "board": {"new": "z"}}
        ]}),
    )
    .await?;
    // A hand edit places the shop on the idea board too; its home stays the
    // context, the board with the lower ID.
    fs::write(
        directory.path().join("abyme/boards/zeta.board.json"),
        r#"{"format": 1, "id": "zeta", "title": "Zeta",
            "zone": {"mode": "free", "statement": "idea"}, "placements": [
            {"element": "shop", "x": 0, "y": 0, "width": 180, "height": 90},
            {"element": "spark", "x": 0, "y": 100, "width": 180, "height": 90}]}"#,
    )?;
    let unavailable = "Review flags unavailable: the project lies in no Git repository\n";

    let board = call(&session, "read_board", json!({"board": "zeta"})).await??;
    let overview = call(&session, "overview", json!({})).await??;
    let shop = call(&session, "read_element", json!({"element": "shop"})).await??;

    assert_eq!(
        text(&board),
        format!(
            "{unavailable}Board: zeta · Zeta\n\
             Zone: free · idea\n\
             \n\
             shop   c4/system  current  Shop\n\
             spark  note       idea     Spark\n"
        )
    );
    assert_eq!(
        text(&overview),
        format!(
            "{unavailable}context  Context  mirrored · current    1 element\n\
             zeta     Zeta     free · idea           1 element\n"
        )
    );
    assert!(
        text(&shop).contains("\nType: c4/system\nStatement kind: current, from board context\n"),
        "{}",
        text(&shop)
    );
    assert!(
        text(&shop).contains("\nBoards: context, zeta\n"),
        "{}",
        text(&shop)
    );
    closed(session).await
}

#[tokio::test]
async fn apply_creates_and_deletes_relations_in_the_element_files() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    let element_file = directory.path().join("abyme/elements/payment-service.md");

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "db", "type_ref": "c4/datastore", "name": "Orders DB"},
            {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "uses",
             "to": {"new": "db"}, "label": "SQL"},
            {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "depends_on",
             "to": {"new": "db"}}
        ]}),
    )
    .await?;
    assert_eq!(
        fs::read_to_string(&element_file)?,
        "---\n\
         id: \"payment-service\"\n\
         type: \"c4/container\"\n\
         name: \"Payment Service\"\n\
         summary: \"Payments through Stripe\"\n\
         relations:\n  \
         - {kind: \"depends_on\", to: \"orders-db\"}\n  \
         - {kind: \"uses\", to: \"orders-db\", label: \"SQL\"}\n\
         ---\n"
    );

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "delete_relation", "from": {"existing": "payment-service"}, "kind": "depends_on",
             "to": {"existing": "orders-db"}}
        ]}),
    )
    .await?;
    let text_after = fs::read_to_string(&element_file)?;
    assert!(!text_after.contains("depends_on"), "{text_after}");
    assert!(text_after.contains("{kind: \"uses\", to: \"orders-db\", label: \"SQL\"}"));

    closed(session).await
}

#[tokio::test]
async fn read_board_shows_relations_with_their_labels_escaped_onto_one_line() -> TestResult {
    let (_directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "db", "type_ref": "c4/datastore", "name": "Orders DB"},
            {"op": "place_element", "element": {"new": "db"}, "board": {"existing": "container-level"}},
            {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "uses",
             "to": {"new": "db"}, "label": "SQL\nBoard: forged \u{202e}x"}
        ]}),
    )
    .await?;

    let read = call_ok(&session, "read_board", json!({"board": "container-level"})).await?;

    assert_eq!(
        read,
        "Review flags unavailable: the project lies in no Git repository\n\
         Board: container-level · Container Level\n\
         \n\
         orders-db        c4/datastore  Orders DB\n\
         payment-service  c4/container  Payment Service  Payments through Stripe\n\
         \n\
         payment-service → orders-db  uses  SQL\\nBoard: forged \\u{202e}x\n"
    );

    closed(session).await
}

#[tokio::test]
async fn read_element_shows_an_element_with_its_relations_and_description() -> TestResult {
    let (_directory, session) = started_project().await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "b", "title": "Container Level"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payment Service",
             "summary": "Payments through Stripe", "body": "Handles payments.\nRefunds\u{202e} run later.\n"},
            {"op": "create_element", "key": "db", "type_ref": "c4/datastore", "name": "Orders DB"},
            {"op": "create_element", "key": "web", "type_ref": "c4/container", "name": "Web Shop"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "b"}},
            {"op": "create_relation", "from": {"new": "pay"}, "kind": "uses", "to": {"new": "db"},
             "label": "SQL"},
            {"op": "create_relation", "from": {"new": "web"}, "kind": "uses", "to": {"new": "pay"},
             "label": "REST"}
        ]}),
    )
    .await?;

    let read = call_ok(
        &session,
        "read_element",
        json!({"element": "payment-service"}),
    )
    .await?;

    assert_eq!(
        read,
        "Review flags unavailable: the project lies in no Git repository\n\
         Element: payment-service · Payment Service\n\
         Type: c4/container\n\
         Statement kind: none\n\
         Anchor: none\n\
         Verified: none\n\
         Summary: Payments through Stripe\n\
         Boards: container-level\n\
         Inner board: none\n\
         \n\
         Outgoing relations:\n\
         payment-service → orders-db  uses  SQL\n\
         \n\
         Incoming relations:\n\
         web-shop → payment-service  uses  REST\n\
         \n\
         Description:\n\
         Handles payments.\n\
         Refunds\\u{202e} run later.\n"
    );

    closed(session).await
}

#[tokio::test]
async fn apply_updates_an_element_and_read_element_shows_it() -> TestResult {
    let (_directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_element", "element": {"existing": "payment-service"}, "name": "Payments",
             "summary": null, "body": "Handles payments.\n"}
        ]}),
    )
    .await?;

    let read = call(
        &session,
        "read_element",
        json!({"element": "payment-service"}),
    )
    .await??;
    assert_eq!(
        text(&read),
        "Review flags unavailable: the project lies in no Git repository\n\
         Element: payment-service · Payments\n\
         Type: c4/container\n\
         Statement kind: none\n\
         Anchor: none\n\
         Verified: none\n\
         Boards: container-level\n\
         Inner board: none\n\
         \n\
         Outgoing relations:\n\
         none\n\
         \n\
         Incoming relations:\n\
         none\n\
         \n\
         Description:\n\
         Handles payments.\n"
    );

    closed(session).await
}

#[tokio::test]
async fn a_rejected_update_is_a_tool_error_and_changes_no_file() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    let before = files(directory.path())?;

    let rejected = call_refused(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_element", "element": {"existing": "payment-service"}, "name": "Renamed"},
            {"op": "update_element", "element": {"existing": "nowhere"}, "name": "x"}
        ]}),
    )
    .await?;

    assert_eq!(rejected, "operation 2: no element has the ID nowhere");
    assert_eq!(files(directory.path())?, before);

    closed(session).await
}

#[tokio::test]
async fn apply_deletes_an_element_and_read_board_no_longer_shows_it() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;

    call_ok(
        &session,
        "apply",
        json!({"operations": [{"op": "delete_element", "element": {"existing": "payment-service"}}]}),
    )
    .await?;

    let read = call(&session, "read_board", json!({"board": "container-level"})).await??;
    assert_eq!(
        text(&read),
        "Review flags unavailable: the project lies in no Git repository\nBoard: container-level · Container Level\n\n"
    );
    assert!(
        !directory
            .path()
            .join("abyme/elements/payment-service.md")
            .exists()
    );

    closed(session).await
}

#[tokio::test]
async fn apply_refuses_to_delete_an_element_that_others_relate_to() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "web", "type_ref": "c4/container", "name": "Web Shop"},
            {"op": "create_relation", "from": {"new": "web"}, "kind": "uses",
             "to": {"existing": "payment-service"}, "label": "SECRET-LABEL"}
        ]}),
    )
    .await?;
    let before = files(directory.path())?;

    let refused = call_refused(
        &session,
        "apply",
        json!({"operations": [{"op": "delete_element", "element": {"existing": "payment-service"}}]}),
    )
    .await?;

    assert_eq!(
        refused,
        "operation 1: element payment-service is still the target of relations from \
         web-shop (uses); delete those first"
    );
    assert_eq!(files(directory.path())?, before);

    closed(session).await
}

#[tokio::test]
async fn read_element_names_an_element_without_a_file() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    fs::remove_file(directory.path().join("abyme/elements/payment-service.md"))?;

    for id in ["nowhere", "payment-service"] {
        let read = call_refused(&session, "read_element", json!({"element": id})).await?;

        assert_eq!(read, format!("no element has the ID {id}"));
    }

    closed(session).await
}

#[tokio::test]
async fn read_element_answers_with_an_error_rather_than_a_cut_detail() -> TestResult {
    let (_directory, session) = started_project().await?;
    let body = "\u{202e}".repeat(300_000);
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "big", "type_ref": "t", "name": "Big", "body": body}
        ]}),
    )
    .await?;

    let read = call_refused(&session, "read_element", json!({"element": "big"})).await?;

    assert!(
        read.starts_with("the detail of element big has "),
        "{read:?}"
    );
    assert!(!read.contains('\u{202e}'), "{read:?}");

    closed(session).await
}

#[tokio::test]
async fn apply_refuses_an_unknown_relation_kind_and_changes_no_file() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    let before = files(directory.path())?;

    let rejected = call_refused(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "contains",
             "to": {"existing": "payment-service"}, "label": "SECRET-LABEL"}
        ]}),
    )
    .await?;

    assert!(rejected.contains("contains"), "{rejected:?}");
    assert!(!rejected.contains("SECRET"), "{rejected:?}");
    assert_eq!(files(directory.path())?, before);

    closed(session).await
}

#[tokio::test]
async fn apply_gives_an_element_a_statement_kind_of_its_own() -> TestResult {
    let (directory, session) = started_project().await?;
    let file = directory.path().join("abyme/elements/note.md");
    let applied = |operation: Value| json!({"operations": [operation]});
    let update = |statement: Value| {
        applied(
            json!({"op": "update_element", "element": {"existing": "note"}, "statement": statement}),
        )
    };

    call_ok(
        &session,
        "apply",
        applied(
            json!({"op": "create_element", "key": "n", "type_ref": "note", "name": "Note",
                       "statement": "question"}),
        ),
    )
    .await?;

    assert!(fs::read_to_string(&file)?.contains("statement: \"question\"\n"));

    call_ok(&session, "apply", update(json!("decision"))).await?;

    assert!(fs::read_to_string(&file)?.contains("statement: \"decision\"\n"));

    let before = fs::read(&file)?;
    call_refused(&session, "apply", update(json!("historic"))).await?;

    assert_eq!(fs::read(&file)?, before);

    call_ok(&session, "apply", update(Value::Null)).await?;

    assert!(!fs::read_to_string(&file)?.contains("statement"));
    closed(session).await
}

#[tokio::test]
async fn an_own_kind_wins_over_the_zone_in_the_read_tools() -> TestResult {
    let (_directory, session) = started_project().await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "b", "title": "B",
             "zone": {"mode": "mirrored", "statement": "current"}},
            {"op": "create_element", "key": "q", "type_ref": "note", "name": "Open point",
             "statement": "question"},
            {"op": "create_element", "key": "f", "type_ref": "note", "name": "Plain fact"},
            {"op": "place_element", "element": {"new": "q"}, "board": {"new": "b"}},
            {"op": "place_element", "element": {"new": "f"}, "board": {"new": "b"}}
        ]}),
    )
    .await?;

    let board = call(&session, "read_board", json!({"board": "b"})).await??;
    let detail = call(&session, "read_element", json!({"element": "open-point"})).await??;
    let plain = call(&session, "read_element", json!({"element": "plain-fact"})).await??;

    let line_of = |id: &str| {
        text(&board)
            .lines()
            .find(|line| line.starts_with(id))
            .map(str::to_owned)
    };
    assert!(
        line_of("open-point").is_some_and(|line| line.contains("question")),
        "{board:?}"
    );
    assert!(
        line_of("plain-fact").is_some_and(|line| line.contains("current")),
        "{board:?}"
    );
    assert!(
        text(&detail).contains("\nStatement kind: question, from the element\n"),
        "{detail:?}"
    );
    assert!(
        text(&plain).contains("\nStatement kind: current, from board b\n"),
        "{plain:?}"
    );

    call_ok(
        &session,
        "apply",
        json!({"operations": [{"op": "update_element", "element": {"existing": "open-point"},
                               "statement": "decision"}]}),
    )
    .await?;
    let detail = call(&session, "read_element", json!({"element": "open-point"})).await??;

    assert!(
        text(&detail).contains("\nStatement kind: decision, from the element\n"),
        "{detail:?}"
    );
    assert!(text(&detail).contains("\nVerified: none\n"), "{detail:?}");
    closed(session).await
}
