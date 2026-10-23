//! Getting a model's files into the user's cache, checked against the size
//! and SHA-256 built into its [`ModelSpec`].
//!
//! Integrity rests on those constants alone. Bytes from the network or the
//! cache are checked in memory before they are written or used, and the
//! checked bytes are the ones the model loads.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ureq::Agent;
use ureq::config::RedirectAuthHeaders;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::cache::{self, CacheEntry, UserCache};
use crate::model::{ModelFile, ModelSpec};

#[cfg(test)]
mod tests;

/// Where model files come from.
pub trait ModelSource {
    /// The bytes of `file` at `spec`'s revision. The caller reads at most
    /// one byte more than the file's size and checks what it reads.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be fetched.
    fn open(&self, spec: &ModelSpec, file: &ModelFile) -> io::Result<Box<dyn Read>>;
}

/// Model files from Hugging Face, over HTTPS only, checked against the
/// platform's root certificates.
#[derive(Debug)]
pub struct HuggingFace {
    agent: Agent,
}

/// Why a model's files could not be fetched or loaded.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    /// The cache directory is not an absolute path.
    #[error("the cache path is not absolute")]
    RelativePath,
    /// A file or directory in the cache could not be used.
    #[error("{}: {source}", .path.display())]
    Io {
        /// The file or directory in the cache.
        path: PathBuf,
        /// What went wrong.
        source: io::Error,
    },
    /// A file could not be downloaded.
    #[error("{file} of {repository} could not be downloaded: {source}")]
    Download {
        /// The model's repository.
        repository: &'static str,
        /// The file's name.
        file: &'static str,
        /// What went wrong.
        source: io::Error,
    },
    /// A downloaded file is not the pinned one.
    #[error("the downloaded {file} of {repository} {mismatch}")]
    Mismatch {
        /// The model's repository.
        repository: &'static str,
        /// The file's name.
        file: &'static str,
        /// How it differs.
        mismatch: Mismatch,
    },
    /// The checked files did not load as a model.
    #[error("the embedding model could not be loaded: {0}")]
    Load(String),
}

/// How bytes differ from a pinned file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mismatch {
    /// Fewer bytes than the file has.
    TooShort {
        /// The number of bytes found.
        found: u64,
    },
    /// More bytes than the file has.
    TooLong,
    /// The right length, but another SHA-256.
    Hash,
}

/// The checked bytes of a model's files.
pub struct ModelBytes {
    pub onnx: Vec<u8>,
    pub tokenizer: Vec<u8>,
    pub config: Vec<u8>,
    pub special_tokens_map: Vec<u8>,
    pub tokenizer_config: Vec<u8>,
}

impl Default for HuggingFace {
    /// A client that speaks only HTTPS, follows at most five redirects
    /// without passing on credentials, and gives up on a stalled connection
    /// without cutting off a slow but steady one.
    fn default() -> Self {
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let agent = Agent::config_builder()
            .tls_config(tls)
            .https_only(true)
            .max_redirects(5)
            .redirect_auth_headers(RedirectAuthHeaders::Never)
            .http_status_as_error(true)
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .timeout_per_read(Some(Duration::from_secs(60)))
            .timeout_global(Some(Duration::from_mins(30)))
            .build()
            .new_agent();
        Self { agent }
    }
}

impl ModelSource for HuggingFace {
    fn open(&self, spec: &ModelSpec, file: &ModelFile) -> io::Result<Box<dyn Read>> {
        let response = self
            .agent
            .get(&url(spec, file))
            .call()
            .map_err(|error| io::Error::other(described(&error)))?;
        Ok(Box::new(response.into_body().into_reader()))
    }
}

impl fmt::Display for Mismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { found } => write!(formatter, "is too short, with {found} bytes"),
            Self::TooLong => formatter.write_str("is longer than the pinned file"),
            Self::Hash => formatter.write_str("does not match its pinned SHA-256"),
        }
    }
}

/// The checked files of `spec`'s model: each from the cache when it matches
/// there, otherwise downloaded from `source` and written to the cache only
/// once it matched. The model's lock is held throughout, so processes that
/// fetch at once download each file once. It blocks the thread.
pub fn fetch(
    cache: &UserCache,
    spec: &'static ModelSpec,
    source: &dyn ModelSource,
) -> Result<ModelBytes, ModelError> {
    if !cache.root().is_absolute() {
        return Err(ModelError::RelativePath);
    }
    let dir = cache.path(CacheEntry::ModelDir(spec));
    cache::create_dir(&dir).map_err(at(&dir))?;
    let lock = cache.path(CacheEntry::ModelLock(spec));
    let _lock = cache::lock_file(&lock).map_err(at(&lock))?;
    let files = &spec.files;
    let bytes_of = |file: &'static ModelFile| checked(cache, spec, file, source);
    Ok(ModelBytes {
        onnx: bytes_of(&files.onnx)?,
        tokenizer: bytes_of(&files.tokenizer)?,
        config: bytes_of(&files.config)?,
        special_tokens_map: bytes_of(&files.special_tokens_map)?,
        tokenizer_config: bytes_of(&files.tokenizer_config)?,
    })
}

/// The URL of `file` at `spec`'s revision, from constants only.
fn url(spec: &ModelSpec, file: &ModelFile) -> String {
    format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        spec.repository, spec.revision, file.name
    )
}

/// What went wrong with a request. Errors are shown as they are only when
/// their text holds no URL and no text the server chose; the exception is a
/// TLS error, which may name the hosts of a certificate that chains to a
/// trusted root but does not fit the host. Any other error can carry a
/// redirect's URL, which holds a signed query, or text from the server,
/// which an AI client would read, so it gets a fixed text.
fn described(error: &ureq::Error) -> String {
    if matches!(
        error,
        ureq::Error::StatusCode(_)
            | ureq::Error::Timeout(_)
            | ureq::Error::Io(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
            | ureq::Error::TooManyRedirects
            | ureq::Error::RedirectFailed
            | ureq::Error::InvalidProxyUrl
            | ureq::Error::Tls(_)
            | ureq::Error::Rustls(_)
    ) {
        error.to_string()
    } else if matches!(
        error,
        ureq::Error::BadUri(_) | ureq::Error::RequireHttpsOnly(_)
    ) {
        "a redirect led to an address that is not allowed".to_owned()
    } else if matches!(error, ureq::Error::ConnectProxyFailed(_)) {
        "the proxy refused the connection".to_owned()
    } else {
        "the server's response could not be used".to_owned()
    }
}

/// The checked bytes of `file`. A leftover of an interrupted write is
/// removed first; the lock guarantees that no other process is writing.
fn checked(
    cache: &UserCache,
    spec: &'static ModelSpec,
    file: &'static ModelFile,
    source: &dyn ModelSource,
) -> Result<Vec<u8>, ModelError> {
    let path = cache.path(CacheEntry::ModelFile(spec, file));
    let part = cache.path(CacheEntry::ModelPart(spec, file));
    remove_if_present(&part)?;
    if let Some(bytes) = cached(&path, file)? {
        return Ok(bytes);
    }
    let bytes = download(spec, file, source)?;
    write_new(&part, &bytes)?;
    fs::rename(&part, &path).map_err(at(&path))?;
    Ok(bytes)
}

/// The bytes at `path` if they are a regular file that matches `file`.
/// Anything else there is removed.
fn cached(path: &Path, file: &ModelFile) -> Result<Option<Vec<u8>>, ModelError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(at(path)(error)),
        // A symlink, FIFO, or device is never opened: opening a FIFO blocks.
        Ok(metadata) if !metadata.is_file() => {
            remove_if_present(path)?;
            return Ok(None);
        }
        Ok(_) => {}
    }
    let handle = File::open(path).map_err(at(path))?;
    if handle.metadata().map_err(at(path))?.is_file() {
        let bytes = read_at_most(handle, file).map_err(at(path))?;
        if mismatch(file, &bytes).is_none() {
            return Ok(Some(bytes));
        }
    }
    remove_if_present(path)?;
    Ok(None)
}

/// The bytes of `file` from `source`, if they match it.
fn download(
    spec: &ModelSpec,
    file: &ModelFile,
    source: &dyn ModelSource,
) -> Result<Vec<u8>, ModelError> {
    let bytes = source
        .open(spec, file)
        .and_then(|reader| read_at_most(reader, file))
        .map_err(|error| ModelError::Download {
            repository: spec.repository,
            file: file.name,
            source: error,
        })?;
    mismatch(file, &bytes).map_or(Ok(bytes), |mismatch| {
        Err(ModelError::Mismatch {
            repository: spec.repository,
            file: file.name,
            mismatch,
        })
    })
}

/// Reads at most one byte more than `file` has, so a longer stream is cut
/// off and still recognized as too long. Only the pinned size, never a
/// length the source states, sizes the buffer.
fn read_at_most(reader: impl Read, file: &ModelFile) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(usize::try_from(file.size).unwrap_or(0));
    reader
        .take(file.size.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// How `bytes` differ from `file`, if they do.
fn mismatch(file: &ModelFile, bytes: &[u8]) -> Option<Mismatch> {
    let found = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    match found.cmp(&file.size) {
        std::cmp::Ordering::Less => Some(Mismatch::TooShort { found }),
        std::cmp::Ordering::Greater => Some(Mismatch::TooLong),
        std::cmp::Ordering::Equal => {
            (cache::sha256_hex(bytes) != file.sha256).then_some(Mismatch::Hash)
        }
    }
}

/// Writes `bytes` to a new file at `path`, readable only by the user, and
/// flushes it to the disk.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), ModelError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .map_err(at(path))
}

/// Removes the file at `path`; a symlink is removed, not followed.
fn remove_if_present(path: &Path) -> Result<(), ModelError> {
    fs::remove_file(path).or_else(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(at(path)(error))
        }
    })
}

/// Turns an I/O error at `path` into a [`ModelError`].
fn at(path: &Path) -> impl FnOnce(io::Error) -> ModelError + '_ {
    move |source| ModelError::Io {
        path: path.to_path_buf(),
        source,
    }
}
