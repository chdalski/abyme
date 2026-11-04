//! Tests of the server without a project or a cache, with a hanging download, and with Git
//! variables set.

use super::*;

/// How long the server may take to exit after its standard input closed.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// A proxy URL for `listener`, which accepts connections and never answers.
fn proxy_url(listener: &TcpListener) -> io::Result<String> {
    Ok(format!("http://{}", listener.local_addr()?))
}

#[tokio::test]
async fn read_element_asks_the_project_repository_whatever_git_variables_say() -> TestResult {
    let directory = repository_with_a_changed_anchor()?;
    let other = tempfile::tempdir()?;
    git(other.path(), &["init", "-q"])?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[
            ("XDG_CONFIG_HOME", config.path().as_os_str()),
            ("GIT_DIR", other.path().join(".git").as_os_str()),
            ("GIT_WORK_TREE", other.path().as_os_str()),
            ("GIT_INDEX_FILE", other.path().join("no-index").as_os_str()),
        ],
    )
    .await?;

    let read = call(&session, "read_element", json!({"element": "x"})).await??;

    assert!(
        text(&read).contains(CHANGED_ANCHOR_REVIEW),
        "{}",
        text(&read)
    );
    closed(session).await
}

/// The server has a cache and a way out to the network here, so the test
/// shows that without a project it uses neither.
#[tokio::test]
async fn without_a_project_every_tool_asks_for_init_and_writes_nothing() -> TestResult {
    let directory = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let proxy = TcpListener::bind("127.0.0.1:0")?;
    proxy.set_nonblocking(true)?;
    let proxy_url = proxy_url(&proxy)?;
    let session = start_with(
        directory.path(),
        &[
            ("XDG_CACHE_HOME", cache.path().as_os_str()),
            ("ALL_PROXY", OsStr::new(&proxy_url)),
        ],
    )
    .await?;

    let applied = call_refused(&session, "apply", create_board_with_element()).await?;
    let read = call_refused(&session, "read_board", json!({"board": "main"})).await?;
    let searched = call_refused(&session, "search", json!({"query": "payments"})).await?;
    let status = call_refused(&session, "status", json!({})).await?;

    for result in [applied, read, searched, status] {
        assert!(result.contains("abyme-mcp init"), "{result:?}");
    }
    closed(session).await?;
    assert!(fs::read_dir(directory.path())?.next().is_none());
    assert!(fs::read_dir(cache.path())?.next().is_none());
    assert_eq!(
        proxy.accept().map_err(|error| error.kind()).err(),
        Some(io::ErrorKind::WouldBlock),
        "the server tried to download"
    );
    Ok(())
}

#[tokio::test]
async fn without_a_cache_search_fails_and_the_other_tools_keep_working() -> TestResult {
    let (_directory, session) = started_project().await?;

    let searched = call_refused(&session, "search", json!({"query": "payments"})).await?;
    call_ok(&session, "apply", create_board_with_element()).await?;
    call_ok(&session, "read_board", json!({"board": "container-level"})).await?;

    assert!(
        searched.contains("search needs a cache directory"),
        "{searched:?}"
    );
    closed(session).await
}

/// The download goes through a proxy that accepts the connection and never
/// answers, so it hangs until the server exits.
#[tokio::test]
async fn exits_when_standard_input_closes_while_the_model_download_hangs() -> TestResult {
    let directory = project()?;
    let cache = tempfile::tempdir()?;
    let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("http://{}", proxy.local_addr()?);
    let session = start_with(
        directory.path(),
        &[
            ("XDG_CACHE_HOME", cache.path().as_os_str()),
            ("ALL_PROXY", OsStr::new(&proxy_url)),
        ],
    )
    .await?;

    let (_connection, _) = tokio::time::timeout(EXIT_TIMEOUT, proxy.accept())
        .await
        .map_err(|_| "the server did not start the download")??;
    let Session {
        client, mut child, ..
    } = session;
    tokio::time::timeout(TIMEOUT, client.cancel()).await??;
    let status = tokio::time::timeout(EXIT_TIMEOUT, child.wait())
        .await
        .map_err(|_| "the server did not exit while the download hung")??;

    assert!(status.success(), "{status}");
    Ok(())
}
