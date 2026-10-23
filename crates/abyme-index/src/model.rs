//! The built-in embedding models: where their files come from, what they
//! must hash to, and how they are loaded.

use abyme_core::EmbeddingModel;
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

/// paraphrase-multilingual-MiniLM-L12-v2, quantized.
///
/// These are the files fastembed 7.2.0 uses for `ParaphraseMLMiniLML12V2Q`,
/// with its pooling and quantization. Revision, sizes, and hashes were
/// checked against Hugging Face's API for this repository.
pub static PARAPHRASE_ML_MINILM_L12_V2_Q: ModelSpec = ModelSpec {
    name: "paraphrase-multilingual-minilm-l12-v2",
    repository: "Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q",
    revision: "faf4aa4225822f3bc6376869cb1164e8e3feedd0",
    files: ModelFiles {
        onnx: ModelFile {
            name: "model_optimized.onnx",
            sha256: "634d0f66c29dc934c8fa72b8a4fe91dd4d420a22f1d82a241058d4316e659a99",
            size: 235_052_644,
        },
        tokenizer: ModelFile {
            name: "tokenizer.json",
            sha256: "fa685fc160bbdbab64058d4fc91b60e62d207e8dc60b9af5c002c5ab946ded00",
            size: 17_083_009,
        },
        config: ModelFile {
            name: "config.json",
            sha256: "c8ec081fdad2df991bf5abbf18418fec7a5cdaa421f60ffb060a30040b8c376f",
            size: 673,
        },
        special_tokens_map: ModelFile {
            name: "special_tokens_map.json",
            sha256: "8c785abebea9ae3257b61681b4e6fd8365ceafde980c21970d001e834cf10835",
            size: 964,
        },
        tokenizer_config: ModelFile {
            name: "tokenizer_config.json",
            sha256: "0666eebf692422757e1dddf3c9fb1ded73ba3dc726c5828671fc89e45bf3609f",
            size: 1416,
        },
    },
    dimension: 384,
    pooling: Pooling::Mean,
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

/// The built-in model that a project's choice names. It only selects one of
/// the statics above; nothing of the project file reaches a path or a URL.
#[must_use]
pub const fn spec(model: EmbeddingModel) -> &'static ModelSpec {
    match model {
        EmbeddingModel::BgeSmallEnV15 => &BGE_SMALL_EN_V15_Q,
        EmbeddingModel::ParaphraseMultilingualMiniLmL12V2 => &PARAPHRASE_ML_MINILM_L12_V2_Q,
    }
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
    use std::ptr;

    /// Every model a project can choose. The match stops compiling when a
    /// model is added, so that it gets listed here too.
    fn every_model() -> [EmbeddingModel; 2] {
        [
            EmbeddingModel::BgeSmallEnV15,
            EmbeddingModel::ParaphraseMultilingualMiniLmL12V2,
        ]
        .map(|model| match model {
            EmbeddingModel::BgeSmallEnV15 | EmbeddingModel::ParaphraseMultilingualMiniLmL12V2 => {
                model
            }
        })
    }

    /// The spec of every model a project can choose.
    fn built_in() -> [&'static ModelSpec; 2] {
        every_model().map(spec)
    }

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
    fn selects_each_built_in_model_by_its_static() {
        assert!(ptr::eq(
            spec(EmbeddingModel::BgeSmallEnV15),
            &raw const BGE_SMALL_EN_V15_Q
        ));
        assert!(ptr::eq(
            spec(EmbeddingModel::ParaphraseMultilingualMiniLmL12V2),
            &raw const PARAPHRASE_ML_MINILM_L12_V2_Q
        ));
        assert!(ptr::eq(
            spec(EmbeddingModel::default()),
            &raw const BGE_SMALL_EN_V15_Q
        ));
    }

    #[test]
    fn gives_each_built_in_model_its_own_name() {
        let specs = built_in();
        for spec in specs {
            assert_eq!(
                specs.iter().filter(|other| other.name == spec.name).count(),
                1,
                "{}",
                spec.name
            );
        }
    }

    #[test]
    fn built_in_models_have_only_safe_constants() {
        for spec in built_in() {
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
