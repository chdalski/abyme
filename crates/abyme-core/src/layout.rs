//! Where the boxes of a board go.

use std::num::NonZeroU32;

use abyme_model::Rect;

pub mod free_space;

/// The size of a box on a board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// The width, never zero.
    pub width: NonZeroU32,
    /// The height, never zero.
    pub height: NonZeroU32,
}

impl Size {
    /// The size of a box: 180 × 90.
    pub const DEFAULT: Self = Self {
        width: NonZeroU32::MIN.saturating_add(179),
        height: NonZeroU32::MIN.saturating_add(89),
    };

    /// A rectangle of this size at `x`, `y`.
    #[must_use]
    pub const fn at(self, x: i32, y: i32) -> Rect {
        Rect {
            x,
            y,
            width: self.width,
            height: self.height,
        }
    }
}
