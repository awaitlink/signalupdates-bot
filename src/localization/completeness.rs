use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Completeness {
    Incomplete,
    LikelyComplete,
    Complete,
}

use Completeness::*;

impl Completeness {
    pub const fn warning_text(&self) -> &'static str {
        match self {
            Incomplete => ":warning: For technical reasons, not all languages may be listed below.",
            LikelyComplete => "For technical reasons, not all languages may be listed below. However, everything from \"Updated language translations.\" commits is listed, so the list is likely complete.",
            Complete => "",
        }
    }
}

impl Default for Completeness {
    fn default() -> Self {
        Incomplete
    }
}
