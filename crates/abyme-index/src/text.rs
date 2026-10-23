//! The text the index embeds and searches for each element.

use abyme_model::Element;

/// The version of [`indexed_text`]'s recipe. It names the index directory,
/// so a changed recipe builds a new index.
pub const RECIPE_VERSION: u32 = 1;

/// The text the index embeds and searches for `element`: its type, name,
/// summary, and description, one per line.
#[must_use]
pub fn indexed_text(element: &Element) -> String {
    [
        element.type_ref().as_str(),
        element.name(),
        element.summary().unwrap_or_default(),
        element.body(),
    ]
    .join("\n")
}
