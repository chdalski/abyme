//! TypeScript types for the identifiers, so that the canvas's UI receives
//! them under the type they cross as: an ID is a plain string.

use ts_rs::{Config, TS};

use crate::id::{Id, IdKind};

impl<K: IdKind> TS for Id<K> {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_config: &Config) -> String {
        "string".to_owned()
    }

    fn inline(config: &Config) -> String {
        Self::name(config)
    }
}
