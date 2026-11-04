//! Tests of boards: inner boards, zones, the overview, coordinates, moves, pins, and layout.

use super::*;

#[tokio::test]
async fn apply_builds_an_element_s_inner_board_and_refuses_a_board_inside_itself() -> TestResult {
    let (directory, session) = started_project().await?;

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "ctx", "title": "Context"},
            {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
            {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
            {"op": "create_board", "key": "inside", "title": "Shop Containers",
             "owner": {"new": "shop"}, "summary": "What the shop runs"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payments"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "inside"}}
        ]}),
    )
    .await?;
    let before = files(directory.path())?;
    let rejected = call_refused(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_board", "board": {"existing": "context"}, "owner": {"existing": "payments"}}
        ]}),
    )
    .await?;

    assert_eq!(
        fs::read_to_string(
            directory
                .path()
                .join("abyme/boards/shop-containers.board.json")
        )?,
        "{\n  \"format\": 1,\n  \"id\": \"shop-containers\",\n  \"title\": \"Shop Containers\",\n  \
         \"owner\": \"shop\",\n  \"summary\": \"What the shop runs\",\n  \"placements\": [\n    \
         {\"element\": \"payments\", \"x\": 0, \"y\": 0, \"width\": 180, \"height\": 90}\n  ]\n}\n"
    );
    assert!(
        !fs::read_to_string(directory.path().join("abyme/boards/context.board.json"))?
            .contains("owner")
    );
    assert_eq!(
        rejected,
        "operation 1: board context would lie inside itself"
    );
    assert_eq!(files(directory.path())?, before);
    closed(session).await
}

#[tokio::test]
async fn apply_sets_changes_and_removes_a_board_s_zone_and_refuses_another_one() -> TestResult {
    let (directory, session) = started_project().await?;
    let file = directory.path().join("abyme/boards/ideas.board.json");
    let board_file = |zone: &str| {
        format!(
            "{{\n  \"format\": 1,\n  \"id\": \"ideas\",\n  \"title\": \"Ideas\",\n{zone}  \
             \"placements\": []\n}}\n"
        )
    };
    let mut written = Vec::new();

    for operation in [
        json!({"op": "create_board", "key": "b", "title": "Ideas",
               "zone": {"mode": "free", "statement": "idea"}}),
        json!({"op": "update_board", "board": {"existing": "ideas"},
               "zone": {"mode": "mirrored", "statement": "current"}}),
        json!({"op": "update_board", "board": {"existing": "ideas"}, "zone": null}),
    ] {
        call_ok(&session, "apply", json!({"operations": [operation]})).await?;
        written.push(fs::read_to_string(&file)?);
    }
    let before = files(directory.path())?;
    let refused = call_refused(
        &session,
        "apply",
        json!({"operations": [{"op": "update_board", "board": {"existing": "ideas"},
               "zone": {"mode": {"mirrored": null}, "statement": "historic\n\u{1b}[31m"}}]}),
    )
    .await?;

    assert_eq!(
        written,
        [
            board_file("  \"zone\": {\"mode\": \"free\", \"statement\": \"idea\"},\n"),
            board_file("  \"zone\": {\"mode\": \"mirrored\", \"statement\": \"current\"},\n"),
            board_file(""),
        ]
    );
    assert!(refused.starts_with("invalid arguments: "), "{refused:?}");
    assert!(!refused.chars().any(char::is_control), "{refused:?}");
    assert_eq!(files(directory.path())?, before);
    closed(session).await
}

#[tokio::test]
async fn read_board_and_read_element_show_inner_boards_also_from_hand_edited_files() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "ctx", "title": "Context"},
            {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
            {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
            {"op": "create_board", "key": "inside", "title": "Shop Containers",
             "owner": {"new": "shop"}, "summary": "Runs\nInside of: forged \u{202e}x"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payments"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "inside"}}
        ]}),
    )
    .await?;
    let boards = directory.path().join("abyme/boards");
    // A hand edit closes a cycle: the context is the inside of payments,
    // which lies inside the shop on the context.
    fs::write(
        boards.join("context.board.json"),
        r#"{"format": 1, "id": "context", "title": "Context", "owner": "payments",
            "placements": [{"element": "shop", "x": 0, "y": 0, "width": 180, "height": 90}]}"#,
    )?;
    fs::write(
        boards.join("orphan.board.json"),
        r#"{"format": 1, "id": "orphan", "title": "Orphan", "owner": "gone", "placements": []}"#,
    )?;
    let unavailable = "Review flags unavailable: the project lies in no Git repository\n";

    let context = call(&session, "read_board", json!({"board": "context"})).await??;
    let inside = call(&session, "read_board", json!({"board": "shop-containers"})).await??;
    let orphan = call(&session, "read_board", json!({"board": "orphan"})).await??;
    let payments = call_ok(&session, "read_element", json!({"element": "payments"})).await?;

    assert_eq!(
        text(&context),
        format!(
            "{unavailable}Board: context · Context\n\
             Inside of: payments · Above: shop-containers ⚠ cycle\n\
             \n\
             shop  c4/system  Shop    ▸ shop-containers\n"
        )
    );
    assert_eq!(
        text(&inside),
        format!(
            "{unavailable}Board: shop-containers · Shop Containers\n\
             Inside of: shop · Above: context ⚠ cycle\n\
             Summary: Runs\\nInside of: forged \\u{{202e}}x\n\
             \n\
             payments  c4/container  Payments    ▸ context\n"
        )
    );
    assert_eq!(
        text(&orphan),
        format!("{unavailable}Board: orphan · Orphan\nInside of: gone ⚠ missing\n\n")
    );
    assert!(
        payments.contains("\nBoards: shop-containers\nInner board: context\n"),
        "{payments}"
    );
    closed(session).await
}

#[tokio::test]
async fn overview_lists_the_boards_as_a_tree_also_from_hand_edited_files() -> TestResult {
    let (directory, session) = started_project().await?;
    let root = directory.path();
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "ctx", "title": "Context", "summary": "The shop"},
            {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
            {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
            {"op": "create_board", "key": "inside", "title": "Shop Containers",
             "owner": {"new": "shop"}, "summary": "Runs"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payments"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "inside"}},
            {"op": "create_board", "key": "parts", "title": "Payment Parts", "owner": {"new": "pay"}}
        ]}),
    )
    .await?;
    // Hand edits make `loop-a` and `loop-b` lie inside each other, with a
    // title that tries to clear the screen and break the line.
    write_element(root, "ia", None, None)?;
    write_element(root, "ib", None, None)?;
    for (id, title, owner, placed) in [
        ("loop-a", "Loop\\u001b[2J\\nA", "ib", "ia"),
        ("loop-b", "Loop B", "ia", "ib"),
    ] {
        fs::write(
            root.join(format!("abyme/boards/{id}.board.json")),
            format!(
                "{{\"format\": 1, \"id\": \"{id}\", \"title\": \"{title}\", \"owner\": \"{owner}\", \
                 \"placements\": [{{\"element\": \"{placed}\", \"x\": 0, \"y\": 0, \"width\": 180, \
                 \"height\": 90}}]}}"
            ),
        )?;
    }

    let overview = call_ok(&session, "overview", json!({})).await?;
    let with_argument = call_refused(&session, "overview", json!({"x": 1})).await?;

    assert_eq!(
        overview,
        [
            "Review flags unavailable: the project lies in no Git repository".to_owned(),
            format!(
                "context{}Context{}The shop  1 element",
                " ".repeat(12),
                " ".repeat(11)
            ),
            format!(
                "  shop-containers  Shop Containers{}Runs{}1 element",
                " ".repeat(3),
                " ".repeat(6)
            ),
            format!(
                "    payment-parts  Payment Parts{}0 elements",
                " ".repeat(15)
            ),
            format!(
                "loop-a{}Loop\\u{{1b}}[2J\\nA{}1 element    Inside of: ib · Above: loop-b ⚠ cycle",
                " ".repeat(13),
                " ".repeat(12)
            ),
            format!(
                "loop-b{}Loop B{}1 element    Inside of: ia · Above: loop-a ⚠ cycle",
                " ".repeat(13),
                " ".repeat(22)
            ),
            String::new(),
        ]
        .join("\n")
    );
    assert!(
        with_argument.starts_with("invalid arguments: "),
        "{with_argument:?}"
    );
    closed(session).await
}

#[tokio::test]
async fn overview_counts_the_flags_of_each_board_s_own_elements() -> TestResult {
    let directory = repository_with_marked_elements()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let overview = call(&session, "overview", json!({})).await??;

    // `ghost` has no file, so four elements count.
    assert_eq!(text(&overview), "b  B    4 elements  ⚠ 1 task, 1 hint\n");
    closed(session).await
}

#[tokio::test]
async fn overview_lists_at_most_fifty_boards() -> TestResult {
    let directory = project()?;
    let boards = directory.path().join("abyme/boards");
    fs::create_dir_all(&boards)?;
    for index in 0..53 {
        fs::write(
            boards.join(format!("b{index:02}.board.json")),
            format!(
                "{{\"format\": 1, \"id\": \"b{index:02}\", \"title\": \"T\", \"placements\": []}}"
            ),
        )?;
    }
    let session = start(directory.path()).await?;

    let overview = call(&session, "overview", json!({})).await??;

    let text = text(&overview);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1 + 50 + 1, "{text}");
    assert_eq!(lines.get(1), Some(&"b00  T    0 elements"));
    assert_eq!(lines.get(50), Some(&"b49  T    0 elements"));
    assert_eq!(lines.last(), Some(&"… and 3 more boards"));
    closed(session).await
}

#[tokio::test]
async fn overview_answers_whatever_the_size_of_a_title() -> TestResult {
    let directory = project()?;
    let boards = directory.path().join("abyme/boards");
    fs::create_dir_all(&boards)?;
    // Escaped in full, this title would take 2.4 MB, more than a reply may
    // hold.
    fs::write(
        boards.join("big.board.json"),
        format!(
            "{{\"format\": 1, \"id\": \"big\", \"title\": \"{}\", \"placements\": []}}",
            "\u{202e}".repeat(300_000)
        ),
    )?;
    let session = start(directory.path()).await?;

    let overview = call_ok(&session, "overview", json!({})).await?;

    assert_eq!(
        overview,
        format!(
            "Review flags unavailable: the project lies in no Git repository\n\
             big  {}…    0 elements\n",
            "\\u{202e}".repeat(80)
        )
    );
    closed(session).await
}

#[tokio::test]
async fn apply_moves_an_element_and_refuses_a_position_out_of_range() -> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    let board_file = directory
        .path()
        .join("abyme/boards/container-level.board.json");
    let move_to = |x: i64, y: i64| {
        json!({"operations": [{"op": "move_element", "element": {"existing": "payment-service"},
                               "board": {"existing": "container-level"}, "x": x, "y": y}]})
    };

    let moved = call_ok(&session, "apply", move_to(400, 200)).await?;

    assert_eq!(moved, "The request created nothing.");
    let board = fs::read_to_string(&board_file)?;
    assert!(
        board.contains(
            r#"{"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90}"#
        ),
        "{board}"
    );

    let before = fs::read(&board_file)?;
    let refused = call_refused(&session, "apply", move_to(1_000_001, 0)).await?;

    assert_eq!(
        refused,
        "operation 1: the position x 1000001, y 0 lies outside -1000000 to 1000000"
    );
    assert_eq!(fs::read(&board_file)?, before);
    closed(session).await
}

#[tokio::test]
async fn apply_moves_a_pinned_element_only_when_the_request_says_what_becomes_of_the_pin()
-> TestResult {
    let (directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    let board_file = directory
        .path()
        .join("abyme/boards/container-level.board.json");
    let move_with = |pin: Value| {
        let mut operation = json!({"op": "move_element", "element": {"existing": "payment-service"},
                                   "board": {"existing": "container-level"}, "x": 400, "y": 200});
        if let (Value::Object(operation), Value::Bool(_)) = (&mut operation, &pin) {
            operation.insert("pinned".to_owned(), pin);
        }
        json!({"operations": [operation]})
    };
    let line = |pin: &str| {
        format!(
            r#"{{"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90{pin}}}"#
        )
    };

    call_ok(&session, "apply", move_with(json!(true))).await?;

    let board = fs::read_to_string(&board_file)?;
    assert!(board.contains(&line(r#", "pinned": true"#)), "{board}");

    let before = fs::read(&board_file)?;
    let refused = call_refused(&session, "apply", move_with(Value::Null)).await?;

    assert!(
        refused.contains("is pinned on board container-level"),
        "{refused:?}"
    );
    assert_eq!(fs::read(&board_file)?, before);

    let coordinates = call(
        &session,
        "read_board",
        json!({"board": "container-level", "coordinates": true}),
    )
    .await??;
    assert!(
        text(&coordinates).contains("@400,200 180x90 pinned"),
        "{coordinates:?}"
    );

    call_ok(&session, "apply", move_with(json!(false))).await?;

    let board = fs::read_to_string(&board_file)?;
    assert!(board.contains(&line("")), "{board}");
    closed(session).await
}

#[tokio::test]
async fn apply_lays_out_a_board_by_its_relations_and_keeps_a_pin() -> TestResult {
    let (directory, session) = started_project().await?;
    let board_file = directory.path().join("abyme/boards/b.board.json");

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "b", "title": "B"},
            {"op": "create_element", "key": "lower", "type_ref": "c4/container", "name": "Lower"},
            {"op": "create_element", "key": "upper", "type_ref": "c4/container", "name": "Upper"},
            {"op": "place_element", "element": {"new": "lower"}, "board": {"new": "b"}},
            {"op": "place_element", "element": {"new": "upper"}, "board": {"new": "b"}},
            {"op": "create_relation", "from": {"new": "upper"}, "kind": "uses", "to": {"new": "lower"}},
            {"op": "layout_board", "board": {"new": "b"}}
        ]}),
    )
    .await?;

    let board = fs::read_to_string(&board_file)?;
    assert!(
        board.contains(r#"{"element": "upper", "x": 0, "y": 0, "width": 180, "height": 90}"#),
        "{board}"
    );
    assert!(
        board.contains(r#"{"element": "lower", "x": 0, "y": 130, "width": 180, "height": 90}"#),
        "{board}"
    );

    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "move_element", "element": {"existing": "lower"}, "board": {"existing": "b"},
             "x": 5000, "y": 6000, "pinned": true},
            {"op": "layout_board", "board": {"existing": "b"}}
        ]}),
    )
    .await?;

    let board = fs::read_to_string(&board_file)?;
    assert!(
        board.contains(
            r#"{"element": "lower", "x": 5000, "y": 6000, "width": 180, "height": 90, "pinned": true}"#
        ),
        "{board}"
    );
    assert!(
        board.contains(r#"{"element": "upper", "x": 0, "y": 0, "width": 180, "height": 90}"#),
        "{board}"
    );
    closed(session).await
}

#[tokio::test]
async fn read_board_shows_coordinates_only_on_request() -> TestResult {
    let (_directory, session) = started_project().await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [{"op": "move_element", "element": {"existing": "payment-service"},
                               "board": {"existing": "container-level"}, "x": 400, "y": 200}]}),
    )
    .await?;

    let with = call_ok(
        &session,
        "read_board",
        json!({"board": "container-level", "coordinates": true}),
    )
    .await?;
    let without = call(&session, "read_board", json!({"board": "container-level"})).await??;

    assert_eq!(
        with,
        "Review flags unavailable: the project lies in no Git repository\n\
         Board: container-level · Container Level\n\
         \n\
         payment-service  @400,200 180x90  c4/container  Payment Service  Payments through Stripe\n"
    );
    assert_eq!(
        text(&without),
        "Review flags unavailable: the project lies in no Git repository\n\
         Board: container-level · Container Level\n\
         \n\
         payment-service  c4/container  Payment Service  Payments through Stripe\n"
    );
    closed(session).await
}
