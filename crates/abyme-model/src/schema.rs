//! JSON Schemas for the identifiers, so that a client such as an AI sees
//! their rules before it sends a request. The checks in this crate stay the
//! only binding ones; a schema is a hint.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};

use crate::id::{Id, IdKind, MAX_ID_LEN};
use crate::object_only::ObjectOnly;

impl<K: IdKind> JsonSchema for Id<K> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Owned(format!("{}_id", K::NOUN))
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^[a-z0-9]+(-[a-z0-9]+)*$",
            "maxLength": MAX_ID_LEN,
            "description": format!(
                "The ID of an existing {}: lowercase ASCII letters and digits joined by single hyphens",
                K::NOUN
            ),
        })
    }
}

impl<T: JsonSchema> JsonSchema for ObjectOnly<T> {
    fn inline_schema() -> bool {
        T::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        T::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        T::schema_id()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        T::json_schema(generator)
    }
}
