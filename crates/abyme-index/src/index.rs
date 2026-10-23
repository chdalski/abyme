//! A project's index for one model, and the lock that every operation on it
//! holds.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use abyme_core::Snapshot;
use abyme_model::ElementId;
use lancedb::Connection;

use crate::cache::{self, CacheEntry, ProjectKey, UserCache};
use crate::changes::changes;
use crate::embedder::{EmbedError, Embedder};
use crate::model::ModelSpec;
use crate::table::{self, AddError, Loaded};
use crate::text::indexed_text;

/// The longest query, in characters. Embedding models read about 512
/// tokens, so a longer query adds nothing but cost.
pub const MAX_QUERY_CHARS: usize = 1000;

/// The most results one search returns.
pub const MAX_RESULTS: usize = 50;

/// One project's index for one model, in the user's cache.
#[derive(Debug)]
pub struct Index {
    cache: UserCache,
    key: ProjectKey,
    spec: &'static ModelSpec,
}

/// The lock of a project's index, held for one operation. Only one holder
/// at a time, in any process, refreshes or searches the index.
#[derive(Debug)]
pub struct IndexLock<'a> {
    index: &'a Index,
    _file: File,
}

/// Why the index could not be brought up to date or searched.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// A path in the cache is not valid UTF-8, which `LanceDB` needs.
    #[error("the cache path is not valid UTF-8")]
    NonUtf8Path,
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
    /// `LanceDB` failed.
    #[error("the search index failed: {0}")]
    Lance(#[from] lancedb::Error),
    /// The rows to add did not form a valid batch.
    #[error("the search index failed: {0}")]
    Rows(#[from] AddError),
    /// The embedding model failed.
    #[error("the embedding model failed: {0}")]
    Embed(#[from] EmbedError),
    /// The embedding model returned another number of vectors than texts.
    #[error("the embedding model returned {found} vectors for {expected} texts")]
    VectorCount {
        /// The number of texts.
        expected: usize,
        /// The number of vectors.
        found: usize,
    },
    /// The embedding model returned a vector of the wrong length.
    #[error("the embedding model returned a vector of {found} values, not {expected}")]
    VectorLength {
        /// The model's dimension.
        expected: usize,
        /// The vector's length.
        found: usize,
    },
    /// The embedding model returned a value that is not a finite number.
    #[error("the embedding model returned a value that is not a finite number")]
    NotFinite,
    /// The model's dimension does not fit the index.
    #[error("the embedding model's dimension {0} is not supported")]
    Dimension(usize),
    /// The query is longer than [`MAX_QUERY_CHARS`].
    #[error("the query has {length} characters, more than {MAX_QUERY_CHARS}")]
    QueryTooLong {
        /// The query's length in characters.
        length: usize,
    },
    /// More results were requested than [`MAX_RESULTS`].
    #[error("at most {MAX_RESULTS} results can be requested, not {requested}")]
    TooManyResults {
        /// The number requested.
        requested: usize,
    },
}

impl Index {
    /// The index of the project whose canonical root is `project_root`, for
    /// `spec`'s model, in `cache`.
    #[must_use]
    pub fn new(cache: UserCache, project_root: &Path, spec: &'static ModelSpec) -> Self {
        Self {
            cache,
            key: ProjectKey::of(project_root),
            spec,
        }
    }

    /// Waits until this caller holds the project's index lock. It blocks
    /// the thread, so call it from a blocking thread, not an async task.
    ///
    /// # Errors
    ///
    /// Returns an [`IndexError`] when the cache directory cannot be created
    /// or the lock file cannot be opened or locked.
    pub fn lock(&self) -> Result<IndexLock<'_>, IndexError> {
        if !self.cache.root().is_absolute() {
            return Err(IndexError::RelativePath);
        }
        create_dir(&self.cache.path(CacheEntry::ProjectDir(&self.key)))?;
        let path = self.cache.path(CacheEntry::Lock(&self.key));
        let file = cache::lock_file(&path).map_err(|source| IndexError::Io { path, source })?;
        Ok(IndexLock {
            index: self,
            _file: file,
        })
    }

    fn table_dir(&self) -> PathBuf {
        self.cache.path(CacheEntry::Table(&self.key, self.spec))
    }

    fn dimension(&self) -> Result<i32, IndexError> {
        i32::try_from(self.spec.dimension).map_err(|_| IndexError::Dimension(self.spec.dimension))
    }
}

impl IndexLock<'_> {
    /// Brings the index up to date with `snapshot`: embeds only elements
    /// whose text changed and removes those that are gone. A table that
    /// cannot be used is rebuilt.
    ///
    /// # Errors
    ///
    /// Returns an [`IndexError`] when the cache, `LanceDB`, or the embedder
    /// fails, or the embedder returns unusable vectors; then the index holds
    /// no wrong rows, at most fewer.
    pub async fn refresh(
        &self,
        snapshot: &Snapshot,
        embedder: &mut (dyn Embedder + Send),
    ) -> Result<(), IndexError> {
        let dimension = self.index.dimension()?;
        let (table, stored) = match table::load(&self.connect().await?, dimension).await? {
            Loaded::Ready(table, stored) => (table, stored),
            Loaded::Damaged => (self.rebuild(dimension).await?, BTreeMap::default()),
        };
        let wanted = snapshot
            .elements
            .iter()
            .map(|(id, element)| (id.clone(), indexed_text(element)))
            .collect();
        let changes = changes(&stored, &wanted);
        let texts: Vec<String> = changes.add.iter().map(|(_, text)| text.clone()).collect();
        let vectors = if texts.is_empty() {
            Vec::new()
        } else {
            checked(
                embedder.embed(&texts)?,
                texts.len(),
                self.index.spec.dimension,
            )?
        };
        table::delete(&table, &changes.remove).await?;
        table::add(&table, &changes.add, vectors, dimension).await?;
        table::update_full_text_index(&table).await?;
        Ok(())
    }

    /// The elements of `snapshot` that best match `query`, best first, at
    /// most `limit` of them. The index only ranks: every returned ID names
    /// an element of `snapshot`, and none appears twice.
    ///
    /// # Errors
    ///
    /// Returns an [`IndexError`] when the query or the limit is too large,
    /// or `LanceDB` or the embedder fails.
    pub async fn search(
        &self,
        snapshot: &Snapshot,
        query: &str,
        embedder: &mut (dyn Embedder + Send),
        limit: usize,
    ) -> Result<Vec<ElementId>, IndexError> {
        let length = query.chars().count();
        if length > MAX_QUERY_CHARS {
            return Err(IndexError::QueryTooLong { length });
        }
        if limit > MAX_RESULTS {
            return Err(IndexError::TooManyResults { requested: limit });
        }
        if limit == 0 || query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let Some(table) = table::open(&self.connect().await?).await? else {
            return Ok(Vec::new());
        };
        if table.count_rows(None).await? == 0 {
            return Ok(Vec::new());
        }
        let vector = checked(
            embedder.embed(&[query.to_owned()])?,
            1,
            self.index.spec.dimension,
        )?
        .into_iter()
        .next()
        .unwrap_or_default();
        let found = table::search(&table, query, vector, limit).await?;
        let mut ids: Vec<ElementId> = Vec::new();
        for id in found {
            let Ok(id) = id.parse::<ElementId>() else {
                continue;
            };
            if snapshot.elements.contains_key(&id) && !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.truncate(limit);
        Ok(ids)
    }

    /// Connects to the table's database directory, creating it. A fresh
    /// connection reads what other processes wrote before this lock.
    async fn connect(&self) -> Result<Connection, IndexError> {
        let dir = self.index.table_dir();
        create_dir(&dir)?;
        let uri = dir.to_str().ok_or(IndexError::NonUtf8Path)?;
        Ok(lancedb::connect(uri).execute().await?)
    }

    /// Deletes the table's database directory and creates an empty table.
    async fn rebuild(&self, dimension: i32) -> Result<lancedb::Table, IndexError> {
        let dir = self.index.table_dir();
        fs::remove_dir_all(&dir).map_err(|source| IndexError::Io {
            path: dir.clone(),
            source,
        })?;
        Ok(table::create(&self.connect().await?, dimension).await?)
    }
}

/// Creates a directory and its parents, each new one readable only by the
/// user.
fn create_dir(path: &Path) -> Result<(), IndexError> {
    cache::create_dir(path).map_err(|source| IndexError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// The vectors, if there is one of `dimension` finite values per text.
fn checked(
    vectors: Vec<Vec<f32>>,
    texts: usize,
    dimension: usize,
) -> Result<Vec<Vec<f32>>, IndexError> {
    if vectors.len() != texts {
        return Err(IndexError::VectorCount {
            expected: texts,
            found: vectors.len(),
        });
    }
    for vector in &vectors {
        if vector.len() != dimension {
            return Err(IndexError::VectorLength {
                expected: dimension,
                found: vector.len(),
            });
        }
        if !vector.iter().all(|value| value.is_finite()) {
            return Err(IndexError::NotFinite);
        }
    }
    Ok(vectors)
}
