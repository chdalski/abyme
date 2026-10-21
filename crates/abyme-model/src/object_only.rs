//! Reading a value only from a JSON object.

use std::fmt;
use std::marker::PhantomData;

use serde::de::value::MapAccessDeserializer;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

/// A `T` read only from a JSON object.
///
/// A derived struct or internally tagged enum also reads the array form
/// (`[1, "main", …]`), so the same text would mean one thing here and
/// something else to every other JSON reader. This wrapper keeps the
/// derive's checks for unknown and duplicate fields and rejects every other
/// shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectOnly<T>(pub T);

/// The visitor behind [`ObjectOnly`]; it accepts nothing but a map.
struct ObjectOnlyVisitor<T>(PhantomData<T>);

impl<T> ObjectOnly<T> {
    /// The value read.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for ObjectOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectOnlyVisitor(PhantomData))
    }
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectOnlyVisitor<T> {
    type Value = ObjectOnly<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(MapAccessDeserializer::new(map)).map(ObjectOnly)
    }
}
