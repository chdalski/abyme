//! Which pages the canvas's window may show: only its own. The webview
//! renders text from the repository, so it neither navigates to another
//! origin nor opens a window of its own.

use tauri::Url;

#[cfg(test)]
mod tests;

/// The scheme and host of the origin that serves the canvas's own page in a
/// built app, without a port. Abyme builds on Unix only, where Tauri serves
/// it from `tauri://localhost`.
const OWN_ORIGIN: (&str, &str) = ("tauri", "localhost");

/// Whether the window may navigate to `url`: a page of the canvas's own
/// origin, or of `dev_url`, the dev server's address, which the caller
/// passes only in a development build. Origins are compared by scheme,
/// host, and port, each on its own.
pub fn is_own_page(url: &Url, dev_url: Option<&Url>) -> bool {
    let (scheme, host) = OWN_ORIGIN;
    let own = url.scheme() == scheme && url.host_str() == Some(host) && url.port().is_none();
    own || dev_url.is_some_and(|dev_url| same_origin(url, dev_url))
}

fn same_origin(url: &Url, other: &Url) -> bool {
    url.scheme() == other.scheme()
        && url.host_str() == other.host_str()
        && url.port_or_known_default() == other.port_or_known_default()
}
