//! The Abyme search index: hybrid search, full text and vectors, over a
//! project's elements.
//!
//! The index is derived from the element files and lives in the user's
//! cache, never in the repository. It only ranks: [`IndexLock::search`]
//! returns element IDs, and callers show the elements from the current
//! files. Embedding goes through the [`Embedder`] trait, so the index can be
//! tested without a model.

mod cache;
mod changes;
mod embedder;
mod index;
mod model;
mod table;
mod text;

pub use cache::{ProjectKey, UserCache};
pub use embedder::{EmbedError, Embedder};
pub use index::{Index, IndexError, IndexLock, MAX_QUERY_CHARS, MAX_RESULTS};
pub use model::ModelSpec;
pub use text::{RECIPE_VERSION, indexed_text};
