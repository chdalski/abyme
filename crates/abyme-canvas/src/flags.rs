//! The review flags the UI shows: only which elements have one, and of which
//! level. The reasons name files and other elements, and stay with the AI and
//! `read_element`.

use abyme_core::{Priority, Review};
use abyme_model::ElementId;
use serde::Serialize;

/// How urgent the review of a flagged element is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum FlagPriority {
    /// To be reviewed.
    Task,
    /// Worth a look.
    Hint,
}

impl From<Priority> for FlagPriority {
    fn from(priority: Priority) -> Self {
        match priority {
            Priority::Task => Self::Task,
            Priority::Hint => Self::Hint,
        }
    }
}

/// An element with a review flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ElementFlag {
    /// The element's ID.
    pub element: ElementId,
    /// The highest priority among the element's flags.
    pub priority: FlagPriority,
}

/// Every flagged element of `review`, in element-ID order.
pub fn element_flags(review: &Review) -> Vec<ElementFlag> {
    review
        .flagged()
        .filter_map(|element| {
            review.priority(element).map(|priority| ElementFlag {
                element: element.clone(),
                priority: priority.into(),
            })
        })
        .collect()
}
