//! The Abyme search index: hybrid search, full text and vectors, over a
//! project's elements.
//!
//! The index is derived from the element files and lives in the user's
//! cache, never in the repository. It only ranks: [`IndexLock::search`]
//! returns element IDs, and callers show the elements from the current
//! files. Embedding goes through the [`Embedder`] trait, so the index can be
//! tested without a model. [`load_model`] provides the real one: it
//! downloads a built-in model's pinned files into the user's cache, checks
//! them against the sizes and hashes in the source, and loads them.

mod cache;
mod changes;
mod download;
mod embedder;
mod index;
mod loader;
mod model;
mod table;
mod text;

pub use cache::{ProjectKey, UserCache};
pub use download::{HuggingFace, Mismatch, ModelError, ModelSource};
pub use embedder::{EmbedError, Embedder};
pub use index::{Index, IndexError, IndexLock, MAX_QUERY_CHARS, MAX_RESULTS};
pub use loader::{LoadedModel, load_model};
pub use model::{BGE_SMALL_EN_V15_Q, ModelFile, ModelFiles, ModelSpec};
pub use text::{RECIPE_VERSION, indexed_text};
