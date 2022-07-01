use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Completeness {
    #[default]
    Incomplete,
    LikelyComplete,
    Complete,
}

use Completeness::*;

impl Completeness {
    pub const fn warning_text(&self) -> &'static str {
        match self {
            Incomplete => ":warning: For technical reasons, not all languages may be listed below.",
            LikelyComplete =>
                "For technical reasons, not all languages may be listed below. However, everything from \"Updated language translations\" and similar commits is listed, so the list is likely complete.",
            Complete => "",
        }
    }
}
