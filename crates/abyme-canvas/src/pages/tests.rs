//! Tests of the pages the window may show.

use super::*;

fn url(text: &str) -> Url {
    text.parse().unwrap()
}

fn dev_url() -> Url {
    url("http://localhost:5173/")
}

#[test]
fn the_built_app_s_own_pages_are_allowed() {
    for page in ["tauri://localhost/", "tauri://localhost/index.html#x"] {
        assert!(is_own_page(&url(page), None), "{page}");
    }
}

#[test]
fn the_dev_server_is_allowed_only_in_a_development_build() {
    let page = url("http://localhost:5173/src/main.tsx");

    assert!(is_own_page(&page, Some(&dev_url())));
    assert!(!is_own_page(&page, None));
}

#[test]
fn every_other_page_is_refused() {
    for page in [
        "http://localhost:5174/",
        "http://localhost.evil.example:5173/",
        "http://localhost:5173@evil.example/",
        "https://localhost:5173/",
        "https://reactflow.dev/",
        "tauri://evil/",
        "http://tauri.localhost/",
        "tauri://localhost:1/",
        "data:text/html,<script>alert(1)</script>",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "about:blank",
        "blob:http://localhost:5173/abc",
    ] {
        assert!(!is_own_page(&url(page), Some(&dev_url())), "{page}");
    }
}
