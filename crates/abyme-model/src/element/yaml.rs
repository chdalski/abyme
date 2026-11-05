//! Reading the frontmatter's YAML into the element's fields.
//!
//! The frontmatter must be one mapping of the known keys to plain values,
//! except `relations`, whose value is a list of mappings of plain values;
//! everything else YAML allows is rejected, so the file means the same to
//! every reader.

use std::collections::BTreeSet;
use std::collections::btree_map::Entry;

use rlsp_yaml_parser::{Document, LoaderBuilder, Node, Schema, Span};

use super::{ElementProblem, Relations};
use crate::id::ElementId;
use crate::relation::RelationKind;
use crate::untrusted::shown;

/// How many characters of a parser message or a key an error shows.
const DETAIL_LIMIT: usize = 200;

/// The key-value pairs of a mapping, in the order of the file.
type Entries = [(Node<Span>, Node<Span>)];

/// The frontmatter's values. Only the relations are checked yet.
pub struct Fields {
    pub id: String,
    pub type_ref: String,
    pub name: String,
    pub summary: Option<String>,
    pub anchor: Option<String>,
    pub statement: Option<String>,
    pub verified: Option<String>,
    pub verified_by: Option<String>,
    pub relations: Relations,
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
    let mut anchor = None;
    let mut statement = None;
    let mut verified = None;
    let mut verified_by = None;
    let mut relations = Relations::new();
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
        if key == "relations" {
            relations = read_relations(value_node)?;
            continue;
        }
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
            "anchor" => &mut anchor,
            "statement" => &mut statement,
            "verified" => &mut verified,
            "verified_by" => &mut verified_by,
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
        anchor,
        statement,
        verified,
        verified_by,
        relations,
    })
}

/// The problems a list of flat mappings reports, so that each list keeps
/// its own. The constructors receive a key already cut to
/// [`DETAIL_LIMIT`].
struct ListProblems {
    /// The value is not a list.
    not_a_list: fn() -> ElementProblem,
    /// An item is not a mapping.
    not_a_mapping: fn() -> ElementProblem,
    /// An item names a key twice.
    duplicate_key: fn(String) -> ElementProblem,
    /// An item's key has a value that is not plain.
    non_scalar_value: fn(String) -> ElementProblem,
    /// An item names a key the list does not know.
    unknown_key: fn(String) -> ElementProblem,
}

/// The problems of `relations`.
const RELATION_PROBLEMS: ListProblems = ListProblems {
    not_a_list: || ElementProblem::RelationsNotAList,
    not_a_mapping: || ElementProblem::RelationNotAMapping,
    duplicate_key: |key| ElementProblem::DuplicateRelationKey { key },
    non_scalar_value: |key| ElementProblem::NonScalarRelationValue { key },
    unknown_key: |key| ElementProblem::UnknownRelationKey { key },
};

/// The items of a list of flat mappings. The caller has checked `node`
/// itself with [`check_node`], and reads each item with [`flat_mapping`].
fn list_items<'a>(
    node: &'a Node<Span>,
    problems: &ListProblems,
) -> Result<&'a [Node<Span>], ElementProblem> {
    match node {
        Node::Sequence { items, .. } => Ok(items),
        Node::Scalar { .. } | Node::Mapping { .. } | Node::Alias { .. } => {
            Err((problems.not_a_list)())
        }
    }
}

/// Reads one item of a list of flat mappings: a mapping of the `keys` to
/// plain values, each at most once, and the value of each key in the order
/// of `keys`. The shape has a fixed depth, so a value that is not plain is
/// rejected without reading into it. Keys are compared after decoding,
/// like the frontmatter's own.
fn flat_mapping<const N: usize>(
    item: &Node<Span>,
    keys: [&str; N],
    problems: &ListProblems,
) -> Result<[Option<String>; N], ElementProblem> {
    check_node(item)?;
    let Node::Mapping { entries, .. } = item else {
        return Err((problems.not_a_mapping)());
    };
    let mut seen = BTreeSet::new();
    let mut values = [const { None }; N];
    for (key_node, value_node) in entries {
        check_node(key_node)?;
        let Node::Scalar { value: key, .. } = key_node else {
            return Err(ElementProblem::NonScalarKey);
        };
        if !seen.insert(key.as_str()) {
            return Err((problems.duplicate_key)(shown(key, DETAIL_LIMIT)));
        }
        check_node(value_node)?;
        let Node::Scalar { value, .. } = value_node else {
            return Err((problems.non_scalar_value)(shown(key, DETAIL_LIMIT)));
        };
        let slot = keys
            .iter()
            .position(|known| known == key)
            .and_then(|position| values.get_mut(position))
            .ok_or_else(|| (problems.unknown_key)(shown(key, DETAIL_LIMIT)))?;
        *slot = Some(value.clone());
    }
    Ok(values)
}

/// Reads the value of `relations`: a list of mappings of plain values, one
/// relation after the other. The caller has checked `node` itself with
/// [`check_node`].
fn read_relations(node: &Node<Span>) -> Result<Relations, ElementProblem> {
    let items = list_items(node, &RELATION_PROBLEMS)?;
    let mut relations = Relations::new();
    for item in items {
        let (kind, to, label) = read_relation(item)?;
        match relations.entry((kind, to)) {
            Entry::Occupied(entry) => {
                return Err(ElementProblem::DuplicateRelation {
                    kind,
                    to: entry.key().1.clone(),
                });
            }
            Entry::Vacant(entry) => {
                entry.insert(label);
            }
        }
    }
    Ok(relations)
}

/// Reads one relation: a mapping of `kind`, `to`, and an optional `label`
/// to plain values.
fn read_relation(
    item: &Node<Span>,
) -> Result<(RelationKind, ElementId, Option<String>), ElementProblem> {
    let [kind, to, label] = flat_mapping(item, ["kind", "to", "label"], &RELATION_PROBLEMS)?;
    let kind = kind.ok_or(ElementProblem::MissingRelationKey { key: "kind" })?;
    let to = to.ok_or(ElementProblem::MissingRelationKey { key: "to" })?;
    let kind =
        RelationKind::from_name(&kind).ok_or_else(|| ElementProblem::UnknownRelationKind {
            kind: shown(&kind, DETAIL_LIMIT),
        })?;
    let to = ElementId::try_from(to).map_err(|error| ElementProblem::InvalidRelationTarget {
        detail: error.to_string(),
    })?;
    Ok((kind, to, label))
}
