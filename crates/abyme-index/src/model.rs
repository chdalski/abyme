//! The identity of an embedding model, as far as the index needs it.

/// A built-in embedding model. Its name and revision name the index
/// directory, so a different model or revision never reads another's
/// vectors.
#[derive(Debug, PartialEq, Eq)]
pub struct ModelSpec {
    /// The model's name, used in paths; fixed in the source.
    pub name: &'static str,
    /// The pinned revision, used in paths; fixed in the source.
    pub revision: &'static str,
    /// The length of the vectors the model returns.
    pub dimension: usize,
}
