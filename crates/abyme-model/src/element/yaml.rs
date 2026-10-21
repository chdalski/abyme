//! Reading the frontmatter's YAML into the element's fields.
//!
//! The frontmatter must be one flat mapping of the known keys to plain
//! values; everything else YAML allows is rejected, so the file means the
//! same to every reader.

use std::collections::BTreeSet;

use rlsp_yaml_parser::{Document, LoaderBuilder, Node, Schema, Span};

use super::ElementProblem;
use crate::untrusted::shown;

/// How many characters of a parser message or a key an error shows.
const DETAIL_LIMIT: usize = 200;

/// The key-value pairs of a mapping, in the order of the file.
type Entries = [(Node<Span>, Node<Span>)];

/// The frontmatter's values, not yet checked.
pub struct Fields {
    pub id: String,
    pub type_ref: String,
    pub name: String,
    pub summary: Option<String>,
}

/// Reads the frontmatter's YAML. The failsafe schema keeps every value a
/// string.
pub fn read_fields(frontmatter: &str) -> Result<Fields, ElementProblem> {
    let documents = LoaderBuilder::new()
        .lossless()
        .schema(Schema::Failsafe)
        .build()
        .load(frontmatter)
        .map_err(|error| ElementProblem::Yaml {
            detail: shown(&error.to_string(), DETAIL_LIMIT),
        })?;
    let document = match documents.as_slice() {
        [document] => document,
        other => return Err(ElementProblem::DocumentCount { count: other.len() }),
    };
    check_document(document)?;
    collect_fields(mapping_entries(&document.root)?)
}

/// Rejects directives and document markers, which the emitter never writes.
const fn check_document(document: &Document<Span>) -> Result<(), ElementProblem> {
    if document.version.is_some() || !document.tags.is_empty() {
        Err(ElementProblem::Directive)
    } else if document.explicit_start || document.explicit_end {
        Err(ElementProblem::DocumentMarker)
    } else {
        Ok(())
    }
}

/// The entries of the root mapping.
fn mapping_entries(root: &Node<Span>) -> Result<&Entries, ElementProblem> {
    check_node(root)?;
    match root {
        Node::Mapping { entries, .. } => Ok(entries),
        Node::Scalar { .. } | Node::Sequence { .. } | Node::Alias { .. } => {
            Err(ElementProblem::NotAMapping)
        }
    }
}

/// Rejects aliases, anchors, and tags written in the file. The loader adds
/// a resolved tag to every node, so only `tag_loc` marks a written one.
fn check_node(node: &Node<Span>) -> Result<(), ElementProblem> {
    if matches!(node, Node::Alias { .. }) {
        Err(ElementProblem::Alias)
    } else if node.anchor().is_some() {
        Err(ElementProblem::Anchor)
    } else if node.tag_loc().is_some() {
        Err(ElementProblem::Tag)
    } else {
        Ok(())
    }
}

/// Collects the known keys' values. Keys are compared after decoding, so
/// `id` and `"id"` are the same key.
fn collect_fields(entries: &Entries) -> Result<Fields, ElementProblem> {
    let mut seen = BTreeSet::new();
    let mut id = None;
    let mut type_ref = None;
    let mut name = None;
    let mut summary = None;
    for (key_node, value_node) in entries {
        check_node(key_node)?;
        let Node::Scalar { value: key, .. } = key_node else {
            return Err(ElementProblem::NonScalarKey);
        };
        if !seen.insert(key.as_str()) {
            return Err(ElementProblem::DuplicateKey {
                key: shown(key, DETAIL_LIMIT),
            });
        }
        check_node(value_node)?;
        let Node::Scalar { value, .. } = value_node else {
            return Err(ElementProblem::NonScalarValue {
                key: shown(key, DETAIL_LIMIT),
            });
        };
        let slot = match key.as_str() {
            "id" => &mut id,
            "type" => &mut type_ref,
            "name" => &mut name,
            "summary" => &mut summary,
            unknown => {
                return Err(ElementProblem::UnknownKey {
                    key: shown(unknown, DETAIL_LIMIT),
                });
            }
        };
        *slot = Some(value.clone());
    }
    Ok(Fields {
        id: id.ok_or(ElementProblem::MissingKey { key: "id" })?,
        type_ref: type_ref.ok_or(ElementProblem::MissingKey { key: "type" })?,
        name: name.ok_or(ElementProblem::MissingKey { key: "name" })?,
        summary,
    })
}
