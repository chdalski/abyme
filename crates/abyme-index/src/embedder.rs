//! Turning texts into vectors.

/// Turns texts into vectors: one vector per text, in the same order.
pub trait Embedder {
    /// The vectors of `texts`, one per text, in order.
    ///
    /// # Errors
    ///
    /// Returns an [`EmbedError`] when the model cannot embed the texts.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
}

/// The model could not embed texts.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct EmbedError(pub String);
