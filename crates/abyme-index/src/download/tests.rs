//! Fetching through a fake source, with small fake files whose sizes and
//! hashes a test spec pins. No test here touches the network or the user's
//! cache.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Cursor, Read};
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use fastembed::{Pooling, QuantizationMode};
use rstest::rstest;
use tempfile::TempDir;

use super::*;
use crate::model::{BGE_SMALL_EN_V15_Q, ModelFiles};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// A model of five small files; the hashes are those of [`CONTENTS`].
static FAKE: ModelSpec = ModelSpec {
    name: "fake",
    repository: "test/fake",
    revision: "0123456789abcdef0123456789abcdef01234567",
    files: ModelFiles {
        onnx: ModelFile {
            name: "graph.onnx",
            sha256: "6140e1c3359508f9426dc105eeb358aef10466f3628a3d5a0119d90e68722783",
            size: 15,
        },
        tokenizer: ModelFile {
            name: "tokenizer.json",
            sha256: "a751a8899c1fbdd966f32516fdd88039c7eb0ab8e184238a1fde7e8452227fdc",
            size: 21,
        },
        config: ModelFile {
            name: "config.json",
            sha256: "1fd7af4283193e7e4a6d039b6ffb6413c315aa123563602047fa8d0976ab834a",
            size: 18,
        },
        special_tokens_map: ModelFile {
            name: "special_tokens_map.json",
            sha256: "c5be5efd8111b27f34d0b6661b6f54deb752fa966b35733048686b9ce8e14fd0",
            size: 26,
        },
        tokenizer_config: ModelFile {
            name: "tokenizer_config.json",
            sha256: "00e71ca7b23f079f6ac08592510a91ada982f0a67976a457029712445d9b9fdc",
            size: 28,
        },
    },
    dimension: 4,
    pooling: Pooling::Cls,
    quantization: QuantizationMode::None,
};

/// The bytes of each file of [`FAKE`].
const CONTENTS: [(&str, &[u8]); 5] = [
    ("graph.onnx", b"fake onnx graph"),
    ("tokenizer.json", br#"{"fake": "tokenizer"}"#),
    ("config.json", br#"{"fake": "config"}"#),
    ("special_tokens_map.json", br#"{"fake": "special tokens"}"#),
    ("tokenizer_config.json", br#"{"fake": "tokenizer config"}"#),
];

/// The environment variables that turn `child_fetches` into the body of a
/// child process.
const CHILD_CACHE: &str = "ABYME_DOWNLOAD_TEST_CACHE";
const CHILD_LOG: &str = "ABYME_DOWNLOAD_TEST_LOG";

/// What the fake source serves for one file.
#[derive(Debug, Clone)]
enum Served {
    Bytes(Vec<u8>),
    Endless,
    OpenFails,
    FailsMidway,
}

/// How a test damages a file's bytes.
#[derive(Debug, Clone, Copy)]
enum Damage {
    SameLengthOtherHash,
    TooShort,
    TooLong,
}

/// Serves the pinned contents, or what a test sets for a file, and records
/// every file it opens.
#[derive(Default)]
struct FakeSource {
    served: BTreeMap<&'static str, Served>,
    opened: RefCell<Vec<&'static str>>,
    endless_read: Rc<Cell<u64>>,
}

/// Serves the pinned contents slowly and logs each file it opens to a file
/// that several processes share.
struct LoggingSource {
    log: PathBuf,
}

/// An endless stream that counts the bytes read from it.
struct Endless(Rc<Cell<u64>>);

/// A stream that fails.
struct Failing;

impl FakeSource {
    fn serving(name: &'static str, served: Served) -> Self {
        Self {
            served: BTreeMap::from([(name, served)]),
            ..Self::default()
        }
    }

    fn opened(&self) -> Vec<&'static str> {
        self.opened.borrow().clone()
    }
}

impl ModelSource for FakeSource {
    fn open(&self, _spec: &ModelSpec, file: &ModelFile) -> io::Result<Box<dyn Read>> {
        self.opened.borrow_mut().push(file.name);
        let served = self
            .served
            .get(file.name)
            .cloned()
            .unwrap_or_else(|| Served::Bytes(content(file.name)));
        match served {
            Served::Bytes(bytes) => Ok(Box::new(Cursor::new(bytes))),
            Served::Endless => Ok(Box::new(Endless(Rc::clone(&self.endless_read)))),
            Served::OpenFails => Err(io::Error::other("http status: 404")),
            Served::FailsMidway => Ok(Box::new(Cursor::new(b"fake".to_vec()).chain(Failing))),
        }
    }
}

impl ModelSource for LoggingSource {
    fn open(&self, _spec: &ModelSpec, file: &ModelFile) -> io::Result<Box<dyn Read>> {
        writeln!(
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(&self.log)?,
            "{}",
            file.name
        )?;
        thread::sleep(Duration::from_millis(100));
        Ok(Box::new(Cursor::new(content(file.name))))
    }
}

impl Read for Endless {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        buffer.fill(b'x');
        let read = u64::try_from(buffer.len()).map_err(io::Error::other)?;
        self.0.set(self.0.get() + read);
        Ok(buffer.len())
    }
}

impl Read for Failing {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "connection reset",
        ))
    }
}

fn content(name: &str) -> Vec<u8> {
    CONTENTS
        .iter()
        .find(|(file, _)| *file == name)
        .map(|(_, bytes)| bytes.to_vec())
        .unwrap()
}

fn damaged(bytes: &[u8], damage: Damage) -> Vec<u8> {
    match damage {
        Damage::SameLengthOtherHash => {
            let mut bytes = bytes.to_vec();
            bytes[0] ^= 1;
            bytes
        }
        Damage::TooShort => bytes[..bytes.len() - 1].to_vec(),
        Damage::TooLong => [bytes, b"!"].concat(),
    }
}

fn setup() -> TestResult<(TempDir, UserCache)> {
    let directory = TempDir::new()?;
    let cache = UserCache::from_env(Some(directory.path().into()), None).ok_or("no cache")?;
    Ok((directory, cache))
}

fn file_path(cache: &UserCache, file: &'static ModelFile) -> PathBuf {
    cache.path(CacheEntry::ModelFile(&FAKE, file))
}

fn part_path(cache: &UserCache, file: &'static ModelFile) -> PathBuf {
    cache.path(CacheEntry::ModelPart(&FAKE, file))
}

fn assert_pinned(bytes: &ModelBytes) {
    assert_eq!(bytes.onnx, content("graph.onnx"));
    assert_eq!(bytes.tokenizer, content("tokenizer.json"));
    assert_eq!(bytes.config, content("config.json"));
    assert_eq!(bytes.special_tokens_map, content("special_tokens_map.json"));
    assert_eq!(bytes.tokenizer_config, content("tokenizer_config.json"));
}

/// Every path under `root`, relative to it, with its permission bits.
fn tree(root: &Path) -> TestResult<BTreeMap<PathBuf, u32>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.is_dir() {
                pending.push(path.clone());
            }
            found.insert(
                path.strip_prefix(root)?.to_path_buf(),
                metadata.permissions().mode() & 0o777,
            );
        }
    }
    Ok(found)
}

#[test]
fn downloads_every_file_and_writes_it_to_the_cache() -> TestResult {
    let (_directory, cache) = setup()?;
    let source = FakeSource::default();

    let bytes = fetch(&cache, &FAKE, &source)?;

    assert_pinned(&bytes);
    assert_eq!(source.opened(), CONTENTS.map(|(name, _)| name));
    for file in FAKE.files.all() {
        assert_eq!(fs::read(file_path(&cache, file))?, content(file.name));
    }
    Ok(())
}

#[test]
fn uses_the_cached_files_without_downloading() -> TestResult {
    let (_directory, cache) = setup()?;
    fetch(&cache, &FAKE, &FakeSource::default())?;
    let source = FakeSource::default();

    let bytes = fetch(&cache, &FAKE, &source)?;

    assert_pinned(&bytes);
    assert_eq!(source.opened(), Vec::<&str>::new());
    Ok(())
}

#[rstest]
fn never_accepts_a_download_that_differs(
    #[values(0, 1, 2, 3, 4)] index: usize,
    #[values(Damage::SameLengthOtherHash, Damage::TooShort, Damage::TooLong)] damage: Damage,
) -> TestResult {
    let (_directory, cache) = setup()?;
    let file = FAKE.files.all()[index];
    let source = FakeSource::serving(
        file.name,
        Served::Bytes(damaged(&content(file.name), damage)),
    );

    let error = fetch(&cache, &FAKE, &source).err();

    assert!(
        matches!(error, Some(ModelError::Mismatch { file: name, .. }) if name == file.name),
        "{error:?}"
    );
    assert!(!file_path(&cache, file).exists());
    assert!(!part_path(&cache, file).exists());
    Ok(())
}

#[rstest]
fn never_accepts_a_cached_file_that_changed(
    #[values(0, 1, 2, 3, 4)] index: usize,
    #[values(Damage::SameLengthOtherHash, Damage::TooShort, Damage::TooLong)] damage: Damage,
) -> TestResult {
    let (_directory, cache) = setup()?;
    fetch(&cache, &FAKE, &FakeSource::default())?;
    let file = FAKE.files.all()[index];
    let path = file_path(&cache, file);
    fs::write(&path, damaged(&content(file.name), damage))?;
    let unavailable = FakeSource::serving(file.name, Served::OpenFails);

    let error = fetch(&cache, &FAKE, &unavailable).err();

    assert!(
        matches!(error, Some(ModelError::Download { file: name, .. }) if name == file.name),
        "{error:?}"
    );
    assert_eq!(unavailable.opened(), vec![file.name]);
    assert!(!path.exists());
    assert!(!part_path(&cache, file).exists());
    Ok(())
}

#[test]
fn replaces_a_changed_cached_file_from_the_source() -> TestResult {
    let (_directory, cache) = setup()?;
    fetch(&cache, &FAKE, &FakeSource::default())?;
    let path = file_path(&cache, &FAKE.files.tokenizer);
    fs::write(&path, b"{\"changed\": \"tokenizer\"}")?;
    let source = FakeSource::default();

    let bytes = fetch(&cache, &FAKE, &source)?;

    assert_pinned(&bytes);
    assert_eq!(fs::read(&path)?, content("tokenizer.json"));
    assert_eq!(source.opened(), vec!["tokenizer.json"]);
    Ok(())
}

#[test]
fn replaces_a_symlink_in_the_cache_without_following_it() -> TestResult {
    let (directory, cache) = setup()?;
    fetch(&cache, &FAKE, &FakeSource::default())?;
    let outside = directory.path().join("outside.json");
    fs::write(&outside, content("config.json"))?;
    let path = file_path(&cache, &FAKE.files.config);
    fs::remove_file(&path)?;
    symlink(&outside, &path)?;
    let source = FakeSource::default();

    fetch(&cache, &FAKE, &source)?;

    assert!(fs::symlink_metadata(&path)?.is_file());
    assert_eq!(fs::read(&outside)?, content("config.json"));
    assert_eq!(source.opened(), vec!["config.json"]);
    Ok(())
}

#[test]
fn never_opens_a_fifo_in_the_cache() -> TestResult {
    let (_directory, cache) = setup()?;
    fetch(&cache, &FAKE, &FakeSource::default())?;
    let path = file_path(&cache, &FAKE.files.onnx);
    fs::remove_file(&path)?;
    assert!(Command::new("mkfifo").arg(&path).status()?.success());
    let (sender, receiver) = mpsc::channel();

    // Opening the FIFO would block forever, so the fetch runs on its own
    // thread and the test waits a bounded time.
    let cache_root = cache.root().parent().ok_or("no parent")?.to_path_buf();
    thread::spawn(move || {
        let cache = UserCache::from_env(Some(cache_root.into()), None);
        let result = cache.map(|cache| {
            let source = FakeSource::default();
            fetch(&cache, &FAKE, &source).map(|_| source.opened())
        });
        let _ = sender.send(result.map(|result| result.map_err(|error| error.to_string())));
    });
    let outcome = receiver.recv_timeout(Duration::from_secs(10))?;

    assert_eq!(outcome, Some(Ok(vec!["graph.onnx"])));
    assert!(fs::symlink_metadata(&path)?.is_file());
    Ok(())
}

#[test]
fn cuts_off_a_download_longer_than_the_file() -> TestResult {
    let (_directory, cache) = setup()?;
    let source = FakeSource::serving("graph.onnx", Served::Endless);

    let error = fetch(&cache, &FAKE, &source).err();

    assert!(
        matches!(
            error,
            Some(ModelError::Mismatch {
                file: "graph.onnx",
                mismatch: Mismatch::TooLong,
                ..
            })
        ),
        "{error:?}"
    );
    assert!(source.endless_read.get() <= FAKE.files.onnx.size + 1);
    assert!(!file_path(&cache, &FAKE.files.onnx).exists());
    Ok(())
}

#[rstest]
#[case::open_fails(Served::OpenFails)]
#[case::stream_fails_midway(Served::FailsMidway)]
fn a_failed_download_leaves_nothing_and_the_next_one_succeeds(
    #[case] served: Served,
) -> TestResult {
    let (_directory, cache) = setup()?;
    let source = FakeSource::serving("tokenizer.json", served);

    let error = fetch(&cache, &FAKE, &source).err();

    assert!(
        matches!(
            error,
            Some(ModelError::Download {
                file: "tokenizer.json",
                ..
            })
        ),
        "{error:?}"
    );
    assert!(!file_path(&cache, &FAKE.files.tokenizer).exists());
    assert!(!part_path(&cache, &FAKE.files.tokenizer).exists());
    assert_pinned(&fetch(&cache, &FAKE, &FakeSource::default())?);
    Ok(())
}

#[test]
fn removes_leftovers_of_an_interrupted_download() -> TestResult {
    let (_directory, cache) = setup()?;
    fs::create_dir_all(cache.path(CacheEntry::ModelDir(&FAKE)))?;
    for file in FAKE.files.all() {
        fs::write(part_path(&cache, file), b"partial")?;
    }

    let bytes = fetch(&cache, &FAKE, &FakeSource::default())?;

    assert_pinned(&bytes);
    for file in FAKE.files.all() {
        assert!(!part_path(&cache, file).exists(), "{}", file.name);
        assert_eq!(fs::read(file_path(&cache, file))?, content(file.name));
    }
    Ok(())
}

#[test]
fn creates_private_files_only_in_the_model_directory() -> TestResult {
    let (directory, cache) = setup()?;

    fetch(&cache, &FAKE, &FakeSource::default())?;

    let model = Path::new("abyme/models/fake/0123456789abcdef0123456789abcdef01234567");
    let mut expected: BTreeMap<PathBuf, u32> = [
        "abyme",
        "abyme/models",
        "abyme/models/fake",
        model.to_str().ok_or("not UTF-8")?,
    ]
    .into_iter()
    .map(|dir| (PathBuf::from(dir), 0o700))
    .collect();
    expected.insert(model.join("lock"), 0o600);
    for (name, _) in CONTENTS {
        expected.insert(model.join(name), 0o600);
    }
    assert_eq!(tree(directory.path())?, expected);
    Ok(())
}

/// Not a test of its own: the body of the child processes that
/// `two_processes_download_each_file_once` starts.
#[test]
fn child_fetches() -> TestResult {
    let (Ok(cache), Ok(log)) = (env::var(CHILD_CACHE), env::var(CHILD_LOG)) else {
        return Ok(());
    };
    let cache = UserCache::from_env(Some(cache.into()), None).ok_or("no cache")?;
    let bytes = fetch(&cache, &FAKE, &LoggingSource { log: log.into() })?;
    assert_pinned(&bytes);
    Ok(())
}

#[test]
fn two_processes_download_each_file_once() -> TestResult {
    let (directory, _cache) = setup()?;
    let log = directory.path().join("opened.log");

    let children: Vec<_> = (0..2)
        .map(|_| {
            Command::new(env::current_exe()?)
                .args([
                    "download::tests::child_fetches",
                    "--exact",
                    "--test-threads=1",
                ])
                .env(CHILD_CACHE, directory.path())
                .env(CHILD_LOG, &log)
                .spawn()
        })
        .collect::<Result<_, _>>()?;
    for mut child in children {
        assert!(child.wait()?.success());
    }

    let mut opened: Vec<String> = fs::read_to_string(&log)?
        .lines()
        .map(str::to_owned)
        .collect();
    opened.sort();
    let mut expected: Vec<String> = CONTENTS.map(|(name, _)| name.to_owned()).to_vec();
    expected.sort();
    assert_eq!(opened, expected);
    Ok(())
}

#[test]
fn builds_the_download_url_from_constants() {
    let spec = &BGE_SMALL_EN_V15_Q;

    assert_eq!(
        url(spec, &spec.files.onnx),
        "https://huggingface.co/Qdrant/bge-small-en-v1.5-onnx-Q/resolve/\
         aa8f8b060edb00e03bfdd08813a2949946c8ba55/model_optimized.onnx"
    );
}

#[rstest]
#[case::refused_redirect(ureq::Error::RequireHttpsOnly(
    "http://cdn.example/x?signature=secret".to_owned()
))]
#[case::bad_redirect_uri(ureq::Error::BadUri("cdn.example/x?signature=secret".to_owned()))]
#[case::proxy_answer(ureq::Error::ConnectProxyFailed("secret".to_owned()))]
#[case::other(ureq::Error::Other(Box::new(io::Error::other("location: secret"))))]
fn never_shows_text_from_the_server_or_a_url(#[case] error: ureq::Error) {
    assert!(
        !described(&error).contains("secret"),
        "{}",
        described(&error)
    );
}

#[test]
fn gives_errors_that_are_not_known_safe_a_fixed_text() -> TestResult {
    let invalid = ureq::http::Uri::try_from("http://cdn.example/ x")
        .err()
        .ok_or("valid")?;
    let error = ureq::Error::Http(invalid.into());

    assert_eq!(described(&error), "the server's response could not be used");
    Ok(())
}

#[test]
fn shows_the_status_of_a_failed_request() {
    assert_eq!(described(&ureq::Error::StatusCode(404)), "http status: 404");
}
