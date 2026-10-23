//! Loading a model from its checked files.

use std::sync::{Mutex, PoisonError};

use fastembed::{InitOptionsUserDefined, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel};
use ort::environment::Environment;

use crate::cache::UserCache;
use crate::download::{self, ModelError, ModelSource};
use crate::embedder::{EmbedError, Embedder};
use crate::model::ModelSpec;

/// The process's ONNX Runtime environment, once built. ONNX Runtime builds
/// one environment per process and never another, even after the first one
/// is dropped, so every model of the process shares this one.
static ENVIRONMENT: Mutex<Option<Environment>> = Mutex::new(None);

/// An embedding model, loaded into this process.
pub struct LoadedModel {
    embedding: TextEmbedding,
}

/// Loads `spec`'s model from the user's cache.
///
/// Each file that is missing there or does not match is downloaded from
/// `source`. The model is built from the very bytes that were checked. It
/// blocks the thread for as long as a download takes, so call it from a
/// blocking thread.
///
/// # Errors
///
/// Returns a [`ModelError`] when the cache cannot be used, a file cannot be
/// downloaded or does not match its pinned size and hash, or the files do
/// not load as a model.
pub fn load_model(
    cache: &UserCache,
    spec: &'static ModelSpec,
    source: &dyn ModelSource,
) -> Result<LoadedModel, ModelError> {
    let bytes = download::fetch(cache, spec, source)?;
    let model = UserDefinedEmbeddingModel::new(
        bytes.onnx,
        TokenizerFiles {
            tokenizer_file: bytes.tokenizer,
            config_file: bytes.config,
            special_tokens_map_file: bytes.special_tokens_map,
            tokenizer_config_file: bytes.tokenizer_config,
        },
    )
    .with_pooling(spec.pooling.clone())
    .with_quantization(spec.quantization);
    let options = InitOptionsUserDefined::new().with_environment(environment()?);
    TextEmbedding::try_new_from_user_defined(model, options)
        .map(|embedding| LoadedModel { embedding })
        .map_err(|error| ModelError::Load(error.to_string()))
}

impl Embedder for LoadedModel {
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.embedding
            .embed(texts, None)
            .map_err(|error| EmbedError(error.to_string()))
    }
}

/// The process's ONNX Runtime environment, built at the first call.
fn environment() -> Result<Environment, ModelError> {
    let mut slot = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);
    let environment = if let Some(environment) = slot.as_ref() {
        environment.clone()
    } else {
        let environment = ort::init()
            .build()
            .map_err(|error| ModelError::Load(error.to_string()))?;
        *slot = Some(environment.clone());
        environment
    };
    drop(slot);
    Ok(environment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_one_onnx_runtime_environment_across_loads() {
        assert!(environment().is_ok());
        assert!(environment().is_ok());
    }
}
