//! The element file: Markdown with a YAML frontmatter that holds the
//! element's ID, type, name, summary, and outgoing relations.
//!
//! The frontmatter is read with `rlsp-yaml-parser` and written by an emitter
//! of our own that double-quotes every value. `type`, `name`, `summary`, and
//! the relations' labels hold only characters the parser can read back, and
//! the frontmatter is at most [`MAX_FRONTMATTER_BYTES`], because the parser
//! rejects other characters even as escapes and parses escapes slowly. An
//! [`Element`] can only be built through these checks, so every element read
//! can be written again, and nothing written is rejected on reading.

use std::collections::BTreeMap;

use crate::id::ElementId;
use crate::relation::RelationKind;

mod frontmatter;
mod yaml;

/// The largest frontmatter, in bytes between the two `---` lines as the
/// emitter writes it.
pub const MAX_FRONTMATTER_BYTES: usize = 64 * 1024;

/// An element's outgoing relations, with the label of each, if it has one.
///
/// An element has at most one relation of a kind to a target, and the
/// relations sort by the kind's name, then the target.
pub type Relations = BTreeMap<(RelationKind, ElementId), Option<String>>;

/// An element: what it is, what it is called, its description, and its
/// relations to other elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    id: ElementId,
    type_ref: TypeRef,
    name: String,
    summary: Option<String>,
    body: String,
    relations: Relations,
}

/// What an element holds besides its ID. Only `type_ref` is checked here,
/// by its own type: [`Element::new`] checks the other fields when it builds
/// an element from them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementFields {
    /// The semantic type.
    pub type_ref: TypeRef,
    /// The name shown on the board.
    pub name: String,
    /// The one-line summary, if there is one.
    pub summary: Option<String>,
    /// The description as Markdown.
    pub body: String,
    /// The outgoing relations.
    pub relations: Relations,
}

/// The semantic type of an element, such as `c4/container`: never empty,
/// and only characters an element file can hold.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeRef(String);

/// An element file that cannot be read, or an element that cannot be
/// written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("element {element}: {problem}")]
pub struct ElementError {
    element: ElementId,
    problem: ElementProblem,
}

/// What is wrong with an element or its file. Text from the file appears
/// only escaped and shortened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ElementProblem {
    /// The file starts with a byte order mark.
    #[error("the file starts with a byte order mark")]
    ByteOrderMark,
    /// The first line is not `---`.
    #[error("the file does not start with a `---` line")]
    MissingOpeningLine,
    /// No `---` line closes the frontmatter.
    #[error("no `---` line closes the frontmatter")]
    MissingClosingLine,
    /// The frontmatter is larger than [`MAX_FRONTMATTER_BYTES`].
    #[error("the frontmatter has {size} bytes, more than {MAX_FRONTMATTER_BYTES}")]
    FrontmatterTooLarge {
        /// The frontmatter's size in bytes.
        size: usize,
    },
    /// The frontmatter is not valid YAML.
    #[error("the frontmatter is not valid YAML: {detail}")]
    Yaml {
        /// The parser's message.
        detail: String,
    },
    /// The frontmatter holds no or several YAML documents.
    #[error("the frontmatter holds {count} YAML documents instead of one")]
    DocumentCount {
        /// The number of documents.
        count: usize,
    },
    /// The frontmatter uses a `%YAML` or `%TAG` directive.
    #[error("the frontmatter uses a YAML directive")]
    Directive,
    /// The frontmatter contains a `---` or `...` document marker.
    #[error("the frontmatter contains a document start or end marker")]
    DocumentMarker,
    /// The frontmatter is not a mapping.
    #[error("the frontmatter is not a mapping of keys to values")]
    NotAMapping,
    /// The frontmatter uses an alias.
    #[error("the frontmatter uses an alias")]
    Alias,
    /// The frontmatter uses an anchor.
    #[error("the frontmatter uses an anchor")]
    Anchor,
    /// The frontmatter uses a tag.
    #[error("the frontmatter uses a tag")]
    Tag,
    /// A key is a list or a mapping.
    #[error("a key is not a plain value")]
    NonScalarKey,
    /// A value is a list or a mapping.
    #[error("the value of {key} is not a plain value")]
    NonScalarValue {
        /// The key, escaped.
        key: String,
    },
    /// A key is not one of `id`, `type`, `name`, and `summary`.
    #[error("the key {key} is unknown")]
    UnknownKey {
        /// The key, escaped.
        key: String,
    },
    /// A key appears twice.
    #[error("the key {key} appears twice")]
    DuplicateKey {
        /// The key, escaped.
        key: String,
    },
    /// A required key is missing.
    #[error("the required key `{key}` is missing")]
    MissingKey {
        /// The missing key.
        key: &'static str,
    },
    /// The `id` is not a valid element ID.
    #[error("{detail}")]
    InvalidId {
        /// Why the ID is invalid, with the value escaped.
        detail: String,
    },
    /// The file's `id` differs from its file name.
    #[error("the file calls itself {found}")]
    IdMismatch {
        /// The ID inside the file.
        found: ElementId,
    },
    /// The type is empty.
    #[error("the type is empty")]
    EmptyType,
    /// A value holds a character that the parser cannot read back.
    #[error("the {field} contains U+{code:04X}, which an element file cannot hold")]
    InvalidCharacter {
        /// The field: `type`, `name`, `summary`, or `label`.
        field: &'static str,
        /// The character's code point.
        code: u32,
    },
    /// The value of `relations` is not a list.
    #[error("the value of `relations` is not a list")]
    RelationsNotAList,
    /// An entry of `relations` is not a mapping.
    #[error("a relation is not a mapping of `kind`, `to`, and `label`")]
    RelationNotAMapping,
    /// A relation's value is a list or a mapping.
    #[error("the value of {key} in a relation is not a plain value")]
    NonScalarRelationValue {
        /// The key, escaped.
        key: String,
    },
    /// A relation's key is not one of `kind`, `to`, and `label`.
    #[error("the key {key} of a relation is unknown")]
    UnknownRelationKey {
        /// The key, escaped.
        key: String,
    },
    /// A relation's key appears twice.
    #[error("the key {key} appears twice in a relation")]
    DuplicateRelationKey {
        /// The key, escaped.
        key: String,
    },
    /// A relation lacks a required key.
    #[error("a relation lacks the required key `{key}`")]
    MissingRelationKey {
        /// The missing key.
        key: &'static str,
    },
    /// A relation's kind is none of the known kinds.
    #[error("{kind} is not a relation kind")]
    UnknownRelationKind {
        /// The kind, escaped.
        kind: String,
    },
    /// A relation's target is not a valid element ID.
    #[error("{detail}")]
    InvalidRelationTarget {
        /// Why the target is invalid, with the value escaped.
        detail: String,
    },
    /// The element has two relations of the same kind to the same target.
    #[error("the relation {kind} to {to} appears twice")]
    DuplicateRelation {
        /// The kind.
        kind: RelationKind,
        /// The target.
        to: ElementId,
    },
    /// A relation points to the element itself.
    #[error("the relation {kind} points to the element itself")]
    SelfRelation {
        /// The kind.
        kind: RelationKind,
    },
}

impl Element {
    /// Builds an element after checking that its file can be written and
    /// read back: `name` and `summary` hold only allowed characters, and
    /// the frontmatter fits [`MAX_FRONTMATTER_BYTES`].
    ///
    /// # Errors
    ///
    /// Returns an [`ElementError`] naming `id` when a rule is broken.
    pub fn new(id: ElementId, fields: ElementFields) -> Result<Self, ElementError> {
        let ElementFields {
            type_ref,
            name,
            summary,
            body,
            relations,
        } = fields;
        let element = Self {
            id,
            type_ref,
            name,
            summary,
            body,
            relations,
        };
        match element.broken_rule() {
            Some(problem) => Err(ElementError {
                element: element.id,
                problem,
            }),
            None => Ok(element),
        }
    }

    /// Reads an element file. `expected` is the ID its file name gives, and
    /// the file's own `id` must match it.
    ///
    /// # Errors
    ///
    /// Returns an [`ElementError`] naming `expected` when the text is not an
    /// element file that follows the rules of this module.
    pub fn from_markdown(expected: &ElementId, text: &str) -> Result<Self, ElementError> {
        let fail = |problem| ElementError {
            element: expected.clone(),
            problem,
        };
        let (frontmatter, body) = frontmatter::split(text).map_err(fail)?;
        let size = frontmatter::normalized_len(frontmatter);
        if size > MAX_FRONTMATTER_BYTES {
            return Err(fail(ElementProblem::FrontmatterTooLarge { size }));
        }
        let fields = yaml::read_fields(frontmatter).map_err(fail)?;
        let id = ElementId::try_from(fields.id).map_err(|error| {
            fail(ElementProblem::InvalidId {
                detail: error.to_string(),
            })
        })?;
        if id != *expected {
            return Err(fail(ElementProblem::IdMismatch { found: id }));
        }
        let type_ref = TypeRef::try_from(fields.type_ref).map_err(fail)?;
        Self::new(
            id,
            ElementFields {
                type_ref,
                name: fields.name,
                summary: fields.summary,
                body: body.to_owned(),
                relations: fields.relations,
            },
        )
    }

    /// Writes the element file: the frontmatter with every value
    /// double-quoted, then the body as it is.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        format!("---\n{}---\n{}", frontmatter::emit(self), self.body)
    }

    /// The element's ID.
    #[must_use]
    pub const fn id(&self) -> &ElementId {
        &self.id
    }

    /// The element's semantic type.
    #[must_use]
    pub const fn type_ref(&self) -> &TypeRef {
        &self.type_ref
    }

    /// The element's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The element's one-line summary, if it has one.
    #[must_use]
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// The element's description as Markdown.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// The element's outgoing relations.
    #[must_use]
    pub const fn relations(&self) -> &Relations {
        &self.relations
    }

    /// The element's fields without its ID, to build a changed element
    /// from them through [`Element::new`].
    #[must_use]
    pub fn into_fields(self) -> ElementFields {
        ElementFields {
            type_ref: self.type_ref,
            name: self.name,
            summary: self.summary,
            body: self.body,
            relations: self.relations,
        }
    }

    /// The first rule the element breaks, if any.
    fn broken_rule(&self) -> Option<ElementProblem> {
        disallowed_character("name", &self.name)
            .or_else(|| {
                self.summary
                    .as_deref()
                    .and_then(|summary| disallowed_character("summary", summary))
            })
            .or_else(|| {
                self.relations
                    .values()
                    .flatten()
                    .find_map(|label| disallowed_character("label", label))
            })
            .or_else(|| {
                self.relations
                    .keys()
                    .find(|(_, to)| *to == self.id)
                    .map(|(kind, _)| ElementProblem::SelfRelation { kind: *kind })
            })
            .or_else(|| {
                let size = frontmatter::emit(self).len();
                (size > MAX_FRONTMATTER_BYTES)
                    .then_some(ElementProblem::FrontmatterTooLarge { size })
            })
    }
}

impl TypeRef {
    /// The type as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TypeRef {
    type Error = ElementProblem;

    fn try_from(value: String) -> Result<Self, ElementProblem> {
        if value.is_empty() {
            return Err(ElementProblem::EmptyType);
        }
        disallowed_character("type", &value).map_or(Ok(Self(value)), Err)
    }
}

impl ElementError {
    /// The element the error is about.
    #[must_use]
    pub const fn element(&self) -> &ElementId {
        &self.element
    }

    /// What is wrong.
    #[must_use]
    pub const fn problem(&self) -> &ElementProblem {
        &self.problem
    }
}

/// The problem for the first character of `text` that an element file
/// cannot hold, if any.
fn disallowed_character(field: &'static str, text: &str) -> Option<ElementProblem> {
    text.chars()
        .find(|character| !is_allowed(*character))
        .map(|character| ElementProblem::InvalidCharacter {
            field,
            code: u32::from(character),
        })
}

/// Whether YAML counts `character` as printable, so the parser reads it
/// back: everything except the C0 controls other than tab, line feed, and
/// carriage return, DEL, the C1 controls other than U+0085, U+FFFE, and
/// U+FFFF.
const fn is_allowed(character: char) -> bool {
    !matches!(
        character,
        '\u{0}'..='\u{8}'
            | '\u{b}'
            | '\u{c}'
            | '\u{e}'..='\u{1f}'
            | '\u{7f}'..='\u{84}'
            | '\u{86}'..='\u{9f}'
            | '\u{fffe}'
            | '\u{ffff}'
    )
}

#[cfg(test)]
mod tests;
