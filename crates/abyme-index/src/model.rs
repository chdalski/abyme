//! The built-in embedding models: where their files come from, what they
//! must hash to, and how they are loaded.

use fastembed::{Pooling, QuantizationMode};

/// BGE small en v1.5, quantized.
///
/// These are the files fastembed 7.2.0 uses for `BGESmallENV15Q`, with its
/// pooling and quantization. Revision, sizes, and hashes were checked
/// against Hugging Face's API for this repository.
pub static BGE_SMALL_EN_V15_Q: ModelSpec = ModelSpec {
    name: "bge-small-en-v1.5",
    repository: "Qdrant/bge-small-en-v1.5-onnx-Q",
    revision: "aa8f8b060edb00e03bfdd08813a2949946c8ba55",
    files: ModelFiles {
        onnx: ModelFile {
            name: "model_optimized.onnx",
            sha256: "51f1bd0addd6e859e42c2c8021a5e5461385bb676a649f4b269aa445449f2431",
            size: 66_465_124,
        },
        tokenizer: ModelFile {
            name: "tokenizer.json",
            sha256: "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66",
            size: 711_396,
        },
        config: ModelFile {
            name: "config.json",
            sha256: "13582bcf2effc85b7bf3d3f5532e686bc1c9ce86bb009d10f0ec33cbe92299dd",
            size: 706,
        },
        special_tokens_map: ModelFile {
            name: "special_tokens_map.json",
            sha256: "5d5b662e421ea9fac075174bb0688ee0d9431699900b90662acd44b2a350503a",
            size: 695,
        },
        tokenizer_config: ModelFile {
            name: "tokenizer_config.json",
            sha256: "0b29c7bfc889e53b36d9dd3e686dd4300f6525110eaa98c76a5dafceb2029f53",
            size: 1242,
        },
    },
    dimension: 384,
    pooling: Pooling::Cls,
    quantization: QuantizationMode::Static,
};

/// A built-in embedding model.
///
/// Every value is fixed in the source. The name and revision name the
/// model's directories in the cache, so a different model or revision never
/// reads another's files or vectors.
#[derive(Debug, PartialEq, Eq)]
pub struct ModelSpec {
    /// The model's name, used in paths.
    pub name: &'static str,
    /// The Hugging Face repository that holds the files.
    pub repository: &'static str,
    /// The pinned revision, a commit of the repository, used in paths.
    pub revision: &'static str,
    /// The files the model loads.
    pub files: ModelFiles,
    /// The length of the vectors the model returns.
    pub dimension: usize,
    /// How the model's token vectors become one vector per text.
    pub pooling: Pooling,
    /// How the model's weights are quantized.
    pub quantization: QuantizationMode,
}

/// The files a model loads: the ONNX graph and the tokenizer's four files.
#[derive(Debug, PartialEq, Eq)]
pub struct ModelFiles {
    /// The ONNX graph.
    pub onnx: ModelFile,
    /// `tokenizer.json`.
    pub tokenizer: ModelFile,
    /// `config.json`.
    pub config: ModelFile,
    /// `special_tokens_map.json`.
    pub special_tokens_map: ModelFile,
    /// `tokenizer_config.json`.
    pub tokenizer_config: ModelFile,
}

/// One file of a model, as the pinned revision holds it.
#[derive(Debug, PartialEq, Eq)]
pub struct ModelFile {
    /// The file's name in the repository and in the cache.
    pub name: &'static str,
    /// The SHA-256 of its bytes, in lowercase hex.
    pub sha256: &'static str,
    /// Its exact length in bytes.
    pub size: u64,
}

impl ModelFiles {
    /// Every file, the ONNX graph first.
    #[must_use]
    pub const fn all(&self) -> [&ModelFile; 5] {
        [
            &self.onnx,
            &self.tokenizer,
            &self.config,
            &self.special_tokens_map,
            &self.tokenizer_config,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::{Component, Path};

    /// Every model a project can choose.
    const BUILT_IN: [&ModelSpec; 1] = [&BGE_SMALL_EN_V15_Q];

    fn lowercase_hex(value: &str, length: usize) -> bool {
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn one_normal_component(name: &str) -> bool {
        let mut components = Path::new(name).components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    }

    fn repository_part(part: &str) -> bool {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    }

    #[test]
    fn built_in_models_have_only_safe_constants() {
        for spec in BUILT_IN {
            assert!(lowercase_hex(spec.revision, 40), "{}", spec.revision);
            assert!(
                one_normal_component(spec.name)
                    && spec.name.bytes().all(|byte| byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || b".-".contains(&byte)),
                "{}",
                spec.name
            );
            let parts: Vec<&str> = spec.repository.split('/').collect();
            assert!(
                matches!(parts.as_slice(), [owner, name] if repository_part(owner) && repository_part(name)),
                "{}",
                spec.repository
            );
            let files = spec.files.all();
            for file in files {
                assert!(lowercase_hex(file.sha256, 64), "{}", file.name);
                assert!(file.size > 0, "{}", file.name);
                assert!(
                    one_normal_component(file.name)
                        && !file.name.contains('/')
                        && file.name != "lock"
                        && Path::new(file.name).extension() != Some(OsStr::new("part")),
                    "{}",
                    file.name
                );
                assert_eq!(
                    files.iter().filter(|other| other.name == file.name).count(),
                    1,
                    "{}",
                    file.name
                );
            }
        }
    }
}
