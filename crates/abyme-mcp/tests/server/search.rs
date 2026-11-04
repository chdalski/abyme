//! Tests of `search` with the real embedding models; they are ignored unless run on purpose.

use super::*;

/// The line that starts a reply of a read tool in a project outside Git.
const NO_REPOSITORY: &str = "Review flags unavailable: the project lies in no Git repository\n";

#[tokio::test]
#[ignore = "downloads the real model from Hugging Face"]
async fn search_finds_elements_by_word_and_by_meaning_with_the_real_model() -> TestResult {
    let directory = project()?;
    let cache = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CACHE_HOME", cache.path().as_os_str())],
    )
    .await?;
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "pay", "type_ref": "c4/container",
             "name": "Payment Service", "summary": "Charges credit cards through Stripe"},
            {"op": "create_element", "key": "mail", "type_ref": "c4/container",
             "name": "Mailer", "summary": "Sends newsletters to customers by email"},
            {"op": "create_element", "key": "stock", "type_ref": "c4/container",
             "name": "Inventory", "summary": "Tracks how many items the warehouse holds"}
        ]}),
    )
    .await?;

    let by_word = call_within(
        &session,
        "search",
        json!({"query": "Stripe"}),
        Duration::from_mins(10),
    )
    .await??;
    let by_meaning = call(
        &session,
        "search",
        json!({"query": "electronic messages for subscribers"}),
    )
    .await??;

    eprintln!(
        "by word:\n{}\nby meaning:\n{}",
        text(&by_word),
        text(&by_meaning)
    );
    assert!(
        text(&by_word).starts_with(&format!("{NO_REPOSITORY}payment-service  ")),
        "{by_word:?}"
    );
    assert!(
        text(&by_meaning).starts_with(&format!("{NO_REPOSITORY}mailer  ")),
        "{by_meaning:?}"
    );
    closed(session).await
}

/// Search marks its lines like a board outline, with the real model.
#[tokio::test]
#[ignore = "downloads the real model from Hugging Face"]
async fn search_marks_the_elements_to_review_with_the_real_model() -> TestResult {
    let directory = repository_with_marked_elements()?;
    let cache = tempfile::tempdir()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[
            ("XDG_CACHE_HOME", cache.path().as_os_str()),
            ("XDG_CONFIG_HOME", config.path().as_os_str()),
        ],
    )
    .await?;

    let found = call_within(
        &session,
        "search",
        json!({"query": "N"}),
        Duration::from_secs(600),
    )
    .await??;

    let reply = text(&found);
    eprintln!("{reply}");
    let line = |id: &str| {
        reply
            .lines()
            .find(|line| line.starts_with(&format!("{id} ")))
            .unwrap_or_default()
            .to_owned()
    };
    assert!(line("changed").ends_with("⚠ task"), "{reply}");
    assert!(line("hinted").ends_with("⚠ hint"), "{reply}");
    assert!(!line("calm").contains('⚠'), "{reply}");
    closed(session).await
}

/// Also embeds an element far longer than the 512 tokens the model reads,
/// which the tokenizer must cut instead of failing the search.
#[tokio::test]
#[ignore = "downloads the real multilingual model from Hugging Face"]
async fn search_finds_an_english_element_through_a_german_query_with_the_multilingual_model()
-> TestResult {
    let directory = project()?;
    fs::write(
        directory.path().join("abyme/abyme.toml"),
        "format = 1\nembedding_model = \"paraphrase-multilingual-minilm-l12-v2\"\n",
    )?;
    let cache = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CACHE_HOME", cache.path().as_os_str())],
    )
    .await?;
    let long_body = "The archive keeps every report of the year. ".repeat(200);
    call_ok(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "pay", "type_ref": "c4/container",
             "name": "Payment Service", "summary": "Charges credit cards through Stripe"},
            {"op": "create_element", "key": "mail", "type_ref": "c4/container",
             "name": "Mailer", "summary": "Sends newsletters to customers by email"},
            {"op": "create_element", "key": "stock", "type_ref": "c4/container",
             "name": "Inventory", "summary": "Tracks how many items the warehouse holds"},
            {"op": "create_element", "key": "archive", "type_ref": "c4/container",
             "name": "Archive", "summary": "Keeps old reports", "body": long_body}
        ]}),
    )
    .await?;

    let found = call_within(
        &session,
        "search",
        json!({"query": "Rundbriefe an Kunden verschicken"}),
        Duration::from_mins(15),
    )
    .await??;

    eprintln!("Rundbriefe an Kunden verschicken:\n{}", text(&found));
    assert_eq!(found.is_error, Some(false), "{found:?}");
    assert!(
        text(&found).starts_with(&format!("{NO_REPOSITORY}mailer  ")),
        "{found:?}"
    );
    let models: Vec<String> = fs::read_dir(cache.path().join("abyme/models"))?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    assert_eq!(models, ["paraphrase-multilingual-minilm-l12-v2"]);
    closed(session).await
}
