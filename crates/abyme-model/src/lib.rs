//! The Abyme format: the identifiers of elements and boards, and the text
//! files that hold them.
//!
//! Everything here is pure: it reads from strings and writes to strings.
//! Where the files live and how they are written safely is the business of
//! `abyme-core`.

mod board;
mod element;
mod id;
mod untrusted;

pub use board::{BOARD_FORMAT, Board, BoardError, Rect};
pub use element::{Element, ElementError, ElementProblem, MAX_FRONTMATTER_BYTES, TypeRef};
pub use id::{BoardId, BoardKind, ElementId, ElementKind, Id, IdError, IdKind, IdRule, MAX_ID_LEN};
