//! The embedding model that `search` uses: loaded once, kept for the
//! session, and replaced when a search needs another one, as after the
//! project file chose another model.

use std::ptr;
use std::sync::{Arc, Mutex};

use abyme_index::{Embedder, HuggingFace, ModelSpec, UserCache, load_model};

/// What `search` answers when there is no cache to keep the model and the
/// index in.
const NO_CACHE: &str = "search needs a cache directory: set XDG_CACHE_HOME or HOME to an \
    absolute path";

/// Loads a built-in model, using the user's cache. The server loads from
/// Hugging Face; tests pass a loader that returns a fake.
pub type Loader = Arc<
    dyn Fn(&'static ModelSpec, &UserCache) -> Result<Box<dyn Embedder + Send>, String>
        + Send
        + Sync,
>;

/// The model of the session, and how to load one.
pub struct Models {
    loader: Loader,
    cache: Option<UserCache>,
    loaded: Mutex<Option<Loaded>>,
}

/// A loaded model and the spec it was loaded for.
struct Loaded {
    spec: &'static ModelSpec,
    embedder: Box<dyn Embedder + Send>,
}

impl Models {
    /// Models loaded by `loader` into `cache`; without a cache, every use
    /// fails.
    pub fn new(loader: Loader, cache: Option<UserCache>) -> Self {
        Self {
            loader,
            cache,
            loaded: Mutex::new(None),
        }
    }

    /// Runs `work` with the cache and `spec`'s model, loading the model
    /// first unless it is the one loaded. It blocks while another thread
    /// loads or uses a model, so call it from a blocking thread. A failed
    /// load, or a panic in `work`, leaves no model, so the next call loads
    /// again.
    pub fn with<T>(
        &self,
        spec: &'static ModelSpec,
        work: impl FnOnce(&UserCache, &mut (dyn Embedder + Send)) -> Result<T, String>,
    ) -> Result<T, String> {
        let cache = self.cache.as_ref().ok_or_else(|| NO_CACHE.to_owned())?;
        let mut slot = self.loaded.lock().unwrap_or_else(|poisoned| {
            self.loaded.clear_poison();
            poisoned.into_inner()
        });
        // A model loaded for another spec is dropped here, before the new
        // one loads, so that the two are never in memory together.
        let kept = slot.take().filter(|loaded| ptr::eq(loaded.spec, spec));
        let mut loaded = match kept {
            Some(loaded) => loaded,
            None => Loaded {
                spec,
                embedder: (self.loader)(spec, cache)?,
            },
        };
        let result = work(cache, loaded.embedder.as_mut());
        *slot = Some(loaded);
        drop(slot);
        result
    }
}

/// Downloads `spec`'s model from Hugging Face into `cache`, unless it is
/// there and matches, and loads it.
pub fn from_hugging_face(
    spec: &'static ModelSpec,
    cache: &UserCache,
) -> Result<Box<dyn Embedder + Send>, String> {
    load_model(cache, spec, &HuggingFace::default())
        .map(|model| Box::new(model) as Box<dyn Embedder + Send>)
        .map_err(|error| error.to_string())
}
