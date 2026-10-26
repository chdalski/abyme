//! The Abyme format: the identifiers of elements and boards, and the text
//! files that hold them.
//!
//! Everything here is pure: it reads from strings and writes to strings.
//! Where the files live and how they are written safely is the business of
//! `abyme-core`.

mod board;
mod element;
mod id;
mod object_only;
mod relation;
#[cfg(feature = "json-schema")]
mod schema;
#[cfg(feature = "ts")]
mod ts;
mod untrusted;

pub use board::{BOARD_FORMAT, Board, BoardError, Rect};
pub use element::{
    Element, ElementError, ElementFields, ElementProblem, MAX_FRONTMATTER_BYTES, Relations, TypeRef,
};
pub use id::{BoardId, BoardKind, ElementId, ElementKind, Id, IdError, IdKind, IdRule, MAX_ID_LEN};
pub use object_only::ObjectOnly;
pub use relation::RelationKind;
pub use untrusted::shown;
