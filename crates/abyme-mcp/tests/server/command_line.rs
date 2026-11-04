//! Tests of `abyme-mcp init` and of the command lines it refuses.

use super::*;

#[test]
fn init_creates_a_project_and_names_the_one_a_directory_lies_in() -> TestResult {
    let directory = tempfile::tempdir()?;

    let created = run(directory.path(), &["init"])?;

    assert!(created.status.success(), "{created:?}");
    let root = fs::canonicalize(directory.path())?;
    let stdout = String::from_utf8(created.stdout)?;
    assert!(
        stdout.starts_with("Created an Abyme project in "),
        "{stdout}"
    );
    assert!(stdout.contains(&*root.to_string_lossy()), "{stdout}");
    assert_eq!(
        fs::read_to_string(directory.path().join("abyme/abyme.toml"))?,
        "format = 1\n"
    );

    let subdirectory = directory.path().join("src");
    fs::create_dir(&subdirectory)?;
    let before = files(directory.path())?;
    let again = run(&subdirectory, &["init"])?;

    assert!(again.status.success(), "{again:?}");
    let stdout = String::from_utf8(again.stdout)?;
    assert!(stdout.contains("nothing changed"), "{stdout}");
    assert!(stdout.contains(&*root.to_string_lossy()), "{stdout}");
    assert_eq!(files(directory.path())?, before);
    Ok(())
}

#[test]
fn refuses_any_other_command_line_without_writing() -> TestResult {
    for arguments in [&["init", "extra"][..], &["--foo"], &["--help"]] {
        let directory = tempfile::tempdir()?;

        let output = run(directory.path(), arguments)?;

        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(String::from_utf8(output.stderr)?.starts_with("usage: abyme-mcp"));
        assert!(
            fs::read_dir(directory.path())?.next().is_none(),
            "{arguments:?}"
        );
    }
    Ok(())
}
