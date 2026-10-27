//! Helpers for tests that run Git in temporary repositories: running Git
//! without the user's or the system's configuration, and committing
//! everything in a repository.
//!
//! The crate depends on no other crate of the workspace, so the tests of
//! every crate, `abyme-core`'s included, can use it.

use std::error::Error;
use std::path::Path;
use std::process::Command;

/// Runs `git` in `directory` with `arguments` and returns what it prints,
/// without trailing whitespace.
///
/// Git runs without the user's or the system's configuration: as a test
/// identity, with `main` as the first branch of a new repository, and
/// without signing commits or tags.
///
/// # Errors
///
/// Returns an error when `git` cannot be started, ends with an error, or
/// prints something that is not UTF-8.
pub fn git(directory: &Path, arguments: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .current_dir(directory)
        .args([
            "-c",
            "user.name=Abyme Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "tag.gpgSign=false",
        ])
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()?;
    if !output.status.success() {
        return Err(format!("git {arguments:?} failed: {output:?}").into());
    }
    Ok(String::from_utf8(output.stdout)?.trim_end().to_owned())
}

/// Adds everything in the repository at `directory` and commits it, also
/// when nothing changed, and returns the new HEAD.
///
/// # Errors
///
/// Returns an error when a Git command fails, as [`git`] does.
pub fn commit_all(directory: &Path) -> Result<String, Box<dyn Error>> {
    git(directory, &["add", "-A"])?;
    git(
        directory,
        &["commit", "-q", "--allow-empty", "-m", "change"],
    )?;
    git(directory, &["rev-parse", "HEAD"])
}
