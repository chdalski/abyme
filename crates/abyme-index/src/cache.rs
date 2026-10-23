//! Every path search uses in the user's cache.
//!
//! The cache lives outside the repository, and every path in it comes from
//! [`CacheEntry`]: fixed names, the hash of a project's root, and the
//! constants of a built-in model. Nothing comes from repository content.

use std::ffi::OsString;
use std::fmt::{self, Write as _};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use crate::model::{ModelFile, ModelSpec};
use crate::text::RECIPE_VERSION;

/// Abyme's directory in the user's cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCache {
    root: PathBuf,
}

/// The key that names a project's directory in the cache: the hex SHA-256
/// of its root path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectKey(String);

/// A path in the user's cache.
#[derive(Debug, Clone, Copy)]
pub enum CacheEntry<'a> {
    /// `index/<key>/`, one project's directory.
    ProjectDir(&'a ProjectKey),
    /// `index/<key>/lock`, the lock of one project's index.
    Lock(&'a ProjectKey),
    /// `index/<key>/<model>-<revision>-r<recipe>/`, the `LanceDB` database of
    /// one project and model.
    Table(&'a ProjectKey, &'static ModelSpec),
    /// `models/<model>/<revision>/`, the files of one model.
    ModelDir(&'static ModelSpec),
    /// `models/<model>/<revision>/lock`, the lock of one model's files.
    ModelLock(&'static ModelSpec),
    /// `models/<model>/<revision>/<file>`, one checked file of a model.
    ModelFile(&'static ModelSpec, &'static ModelFile),
    /// `models/<model>/<revision>/<file>.part`, one file while it is written.
    ModelPart(&'static ModelSpec, &'static ModelFile),
}

impl UserCache {
    /// Abyme's directory in the user's cache: `$XDG_CACHE_HOME/abyme` when
    /// that is an absolute path, otherwise `$HOME/.cache/abyme` when `HOME`
    /// is one, otherwise none. Relative values are ignored, as the XDG Base
    /// Directory specification asks, so the cache never lands in the
    /// working directory.
    #[must_use]
    pub fn from_env(xdg_cache_home: Option<OsString>, home: Option<OsString>) -> Option<Self> {
        let absolute =
            |value: Option<OsString>| value.map(PathBuf::from).filter(|path| path.is_absolute());
        absolute(xdg_cache_home)
            .or_else(|| absolute(home).map(|home| home.join(".cache")))
            .map(|cache| Self {
                root: cache.join("abyme"),
            })
    }

    /// Abyme's directory in the cache.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path of `entry`.
    pub(crate) fn path(&self, entry: CacheEntry<'_>) -> PathBuf {
        match entry {
            CacheEntry::ProjectDir(key) => self.root.join("index").join(&key.0),
            CacheEntry::Lock(key) => self.path(CacheEntry::ProjectDir(key)).join("lock"),
            CacheEntry::Table(key, spec) => self
                .path(CacheEntry::ProjectDir(key))
                .join(format!("{}-{}-r{RECIPE_VERSION}", spec.name, spec.revision)),
            CacheEntry::ModelDir(spec) => {
                self.root.join("models").join(spec.name).join(spec.revision)
            }
            CacheEntry::ModelLock(spec) => self.path(CacheEntry::ModelDir(spec)).join("lock"),
            CacheEntry::ModelFile(spec, file) => {
                self.path(CacheEntry::ModelDir(spec)).join(file.name)
            }
            CacheEntry::ModelPart(spec, file) => self
                .path(CacheEntry::ModelDir(spec))
                .join(format!("{}.part", file.name)),
        }
    }
}

impl ProjectKey {
    /// The key of the project whose root is `root`, which must be the
    /// canonical path, so that one project always gets one key.
    #[must_use]
    pub fn of(root: &Path) -> Self {
        Self(sha256_hex(root.as_os_str().as_bytes()))
    }
}

impl fmt::Display for ProjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The SHA-256 of `bytes` in lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    // Writing to a String cannot fail.
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// Creates a directory in the cache and its parents, each new one readable
/// only by the user.
pub fn create_dir(path: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(path)
}

/// Opens the lock file at `path`, creating it readable only by the user,
/// and waits until this caller holds its lock. It blocks the thread.
pub fn lock_file(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    file.lock()?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::{self, ModelError, ModelSource};
    use crate::index::{Index, IndexError};
    use crate::model::BGE_SMALL_EN_V15_Q;
    use rstest::rstest;
    use std::io::Read;

    /// A source that has no files.
    struct NoSource;

    impl ModelSource for NoSource {
        fn open(&self, _spec: &ModelSpec, _file: &ModelFile) -> io::Result<Box<dyn Read>> {
            Err(io::Error::other("no files"))
        }
    }

    #[rstest]
    #[case::absolute_xdg(Some("/x"), Some("/h"), Some("/x/abyme"))]
    #[case::relative_xdg(Some(".cache"), Some("/h"), Some("/h/.cache/abyme"))]
    #[case::dot_xdg(Some("."), Some("/h"), Some("/h/.cache/abyme"))]
    #[case::empty_xdg(Some(""), Some("/h"), Some("/h/.cache/abyme"))]
    #[case::no_xdg(None, Some("/h"), Some("/h/.cache/abyme"))]
    #[case::relative_home(None, Some("home"), None)]
    #[case::empty_home(Some(""), Some(""), None)]
    #[case::relative_db_prefix(Some("dbcache"), None, None)]
    #[case::nothing(None, None, None)]
    fn finds_the_cache_only_at_absolute_paths(
        #[case] xdg: Option<&str>,
        #[case] home: Option<&str>,
        #[case] expected: Option<&str>,
    ) {
        let cache = UserCache::from_env(xdg.map(OsString::from), home.map(OsString::from));

        assert_eq!(cache.map(|cache| cache.root), expected.map(PathBuf::from));
    }

    #[test]
    fn refuses_a_relative_cache_before_lancedb_sees_it() {
        let cache = UserCache {
            root: PathBuf::from("dbcache/abyme"),
        };
        let index = Index::new(cache, Path::new("/p"), &BGE_SMALL_EN_V15_Q);

        assert!(matches!(index.lock(), Err(IndexError::RelativePath)));
        assert!(!Path::new("dbcache").exists());
    }

    #[test]
    fn refuses_a_relative_cache_before_fetching_a_model() {
        let cache = UserCache {
            root: PathBuf::from("dbcache/abyme"),
        };

        let error = download::fetch(&cache, &BGE_SMALL_EN_V15_Q, &NoSource).err();

        assert!(matches!(error, Some(ModelError::RelativePath)), "{error:?}");
        assert!(!Path::new("dbcache").exists());
    }

    #[test]
    fn keys_a_project_by_the_hash_of_its_root() {
        let key = ProjectKey::of(Path::new("/work/project"));

        assert_eq!(key.0.len(), 64);
        assert!(
            key.0
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );
        assert_eq!(key, ProjectKey::of(Path::new("/work/project")));
        assert_ne!(key, ProjectKey::of(Path::new("/work/other")));
    }

    #[test]
    fn builds_paths_from_the_key_and_the_model_only() {
        let spec = &BGE_SMALL_EN_V15_Q;
        let cache = UserCache::from_env(Some("/c".into()), None).unwrap();
        let key = ProjectKey::of(Path::new("/p"));
        let models = "/c/abyme/models/bge-small-en-v1.5/aa8f8b060edb00e03bfdd08813a2949946c8ba55";

        assert_eq!(
            cache.path(CacheEntry::Lock(&key)),
            PathBuf::from(format!("/c/abyme/index/{key}/lock"))
        );
        assert_eq!(
            cache.path(CacheEntry::Table(&key, spec)),
            PathBuf::from(format!(
                "/c/abyme/index/{key}/bge-small-en-v1.5-aa8f8b060edb00e03bfdd08813a2949946c8ba55-r1"
            ))
        );
        assert_eq!(
            cache.path(CacheEntry::ModelDir(spec)),
            PathBuf::from(models)
        );
        assert_eq!(
            cache.path(CacheEntry::ModelLock(spec)),
            PathBuf::from(format!("{models}/lock"))
        );
        assert_eq!(
            cache.path(CacheEntry::ModelFile(spec, &spec.files.onnx)),
            PathBuf::from(format!("{models}/model_optimized.onnx"))
        );
        assert_eq!(
            cache.path(CacheEntry::ModelPart(spec, &spec.files.tokenizer)),
            PathBuf::from(format!("{models}/tokenizer.json.part"))
        );
    }
}
