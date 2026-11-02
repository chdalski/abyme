//! The Abyme format: the identifiers of elements and boards, and the text
//! files that hold them.
//!
//! Everything here is pure: it reads from strings and writes to strings.
//! Where the files live and how they are written safely is the business of
//! `abyme-core`.

mod anchor;
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
mod verification;
mod zone;

pub use anchor::{Anchor, AnchorRule, MAX_ANCHOR_BYTES, MAX_SEGMENT_BYTES};
pub use board::{BOARD_FORMAT, Board, BoardError, Placement, Rect};
pub use element::{
    Element, ElementError, ElementFields, ElementProblem, MAX_FRONTMATTER_BYTES, NOTE_TYPE,
    Relations, TypeRef, is_allowed,
};
pub use id::{BoardId, BoardKind, ElementId, ElementKind, Id, IdError, IdKind, IdRule, MAX_ID_LEN};
pub use object_only::ObjectOnly;
pub use relation::RelationKind;
pub use untrusted::{FORMAT_AND_SEPARATORS, hides_or_breaks_text, shown};
pub use verification::{CommitId, Verification, Verifier};
pub use zone::{Mode, StatementKind, Zone};
