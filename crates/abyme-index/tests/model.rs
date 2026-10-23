//! The real model, downloaded from Hugging Face into a temporary cache and
//! loaded. Ignored by default because it needs the network; run it with
//! `cargo test -p abyme-index --test model -- --ignored`.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use abyme_index::{
    BGE_SMALL_EN_V15_Q, Embedder, HuggingFace, ModelFile, ModelSource, ModelSpec, UserCache,
    load_model,
};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// A source for a machine without network: every file must come from the
/// cache.
struct Offline;

impl ModelSource for Offline {
    fn open(&self, _spec: &ModelSpec, file: &ModelFile) -> io::Result<Box<dyn Read>> {
        Err(io::Error::other(format!(
            "offline, {} not cached",
            file.name
        )))
    }
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let dot: f32 = left.iter().zip(right).map(|(a, b)| a * b).sum();
    let norm = |vector: &[f32]| vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    dot / (norm(left) * norm(right))
}

/// Every path under `root`, relative to it.
fn tree(root: &Path) -> TestResult<BTreeSet<PathBuf>> {
    let mut found = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if fs::symlink_metadata(&path)?.is_dir() {
                pending.push(path.clone());
            }
            found.insert(path.strip_prefix(root)?.to_path_buf());
        }
    }
    Ok(found)
}

#[test]
#[ignore = "downloads the real model from Hugging Face"]
fn the_real_model_places_related_texts_closer() -> TestResult {
    let directory = TempDir::new()?;
    let cache = UserCache::from_env(Some(directory.path().into()), None).ok_or("no cache")?;

    let mut model = load_model(&cache, &BGE_SMALL_EN_V15_Q, &HuggingFace::default())?;
    let texts = [
        "The cat sleeps on the sofa.",
        "A kitten naps on the couch.",
        "Quarterly tax returns are due in April.",
    ]
    .map(str::to_owned);
    let vectors = model.embed(&texts)?;

    let [cat, kitten, taxes] = vectors.as_slice() else {
        return Err(format!("{} vectors for three texts", vectors.len()).into());
    };
    assert_eq!(cat.len(), BGE_SMALL_EN_V15_Q.dimension);
    let (related, unrelated) = (cosine(cat, kitten), cosine(cat, taxes));
    eprintln!("cosine of the related texts {related}, of the unrelated ones {unrelated}");
    assert!(related > unrelated);

    let model_dir = Path::new("abyme/models/bge-small-en-v1.5")
        .join("aa8f8b060edb00e03bfdd08813a2949946c8ba55");
    let mut expected: BTreeSet<PathBuf> =
        ["abyme", "abyme/models", "abyme/models/bge-small-en-v1.5"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
    expected.insert(model_dir.clone());
    expected.insert(model_dir.join("lock"));
    for file in BGE_SMALL_EN_V15_Q.files.all() {
        expected.insert(model_dir.join(file.name));
    }
    assert_eq!(tree(directory.path())?, expected);
    assert!(!Path::new(".fastembed_cache").exists());

    let mut cached = load_model(&cache, &BGE_SMALL_EN_V15_Q, &Offline)?;
    let again = cached.embed(&texts[..1])?;
    let [cat_again] = again.as_slice() else {
        return Err(format!("{} vectors for one text", again.len()).into());
    };
    assert!(cosine(cat, cat_again) > 0.999);
    Ok(())
}
