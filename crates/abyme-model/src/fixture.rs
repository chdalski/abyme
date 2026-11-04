//! A builder of elements for the tests of other crates, so that a new field
//! of [`ElementFields`] changes one place instead of every test that builds
//! an element.

use crate::{Element, ElementError, ElementFields, ElementId, RelationKind, Relations, TypeRef};

/// An element for a test: of the type `t`, named after its ID, with no
/// summary, body, or relations until a method gives it one, and never an
/// anchor, a statement kind, or a stamp.
#[derive(Debug, Clone)]
pub struct ElementFixture {
    id: String,
    type_ref: String,
    name: String,
    summary: Option<String>,
    body: String,
    relations: Vec<(RelationKind, String, Option<String>)>,
}

impl ElementFixture {
    /// The element `id`.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            type_ref: "t".to_owned(),
            name: id.to_owned(),
            summary: None,
            body: String::new(),
            relations: Vec::new(),
        }
    }

    /// The element with the type `type_ref`.
    #[must_use]
    pub fn type_ref(self, type_ref: &str) -> Self {
        Self {
            type_ref: type_ref.to_owned(),
            ..self
        }
    }

    /// The element with the name `name`.
    #[must_use]
    pub fn name(self, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..self
        }
    }

    /// The element with the summary `summary`, or none.
    #[must_use]
    pub fn summary(self, summary: Option<&str>) -> Self {
        Self {
            summary: summary.map(str::to_owned),
            ..self
        }
    }

    /// The element with the description `body`.
    #[must_use]
    pub fn body(self, body: &str) -> Self {
        Self {
            body: body.to_owned(),
            ..self
        }
    }

    /// The element with `relations`, each of a kind, to an element, and with
    /// a label or none.
    #[must_use]
    pub fn relations(self, relations: &[(RelationKind, &str, Option<&str>)]) -> Self {
        Self {
            relations: relations
                .iter()
                .map(|(kind, to, label)| (*kind, (*to).to_owned(), label.map(str::to_owned)))
                .collect(),
            ..self
        }
    }

    /// The element, or why [`Element::new`] refuses it.
    ///
    /// # Errors
    ///
    /// Returns what [`Element::new`] returns for these fields.
    ///
    /// # Panics
    ///
    /// Panics if the ID, the type, or the ID of a related element is not
    /// valid: a fixture that cannot be read is a mistake in the test.
    #[expect(
        clippy::expect_used,
        reason = "a fixture whose ID or type is invalid is a mistake in the test, not a case it covers"
    )]
    pub fn try_build(self) -> Result<Element, ElementError> {
        let id = |value: &str| -> ElementId { value.parse().expect("a valid element ID") };
        Element::new(
            id(&self.id),
            ElementFields {
                type_ref: TypeRef::try_from(self.type_ref).expect("a valid type"),
                name: self.name,
                summary: self.summary,
                body: self.body,
                anchor: None,
                statement: None,
                verified: None,
                relations: self
                    .relations
                    .into_iter()
                    .map(|(kind, to, label)| ((kind, id(&to)), label))
                    .collect::<Relations>(),
            },
        )
    }

    /// The element.
    ///
    /// # Panics
    ///
    /// Panics if [`ElementFixture::try_build`] does, or if [`Element::new`]
    /// refuses the element.
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "a fixture that Element::new refuses is a mistake in the test, not a case it covers"
    )]
    pub fn build(self) -> Element {
        self.try_build().expect("a valid element")
    }
}
