use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Completeness {
    #[default]
    Incomplete,
    LikelyComplete,
    Complete,
}

use Completeness::*;

use crate::platform::Platform;

impl Completeness {
    pub fn warning_text(&self, platform: Platform) -> String {
        match self {
            Incomplete => String::from(":warning: For technical reasons, not all languages may be listed below."),
            LikelyComplete => format!(
                "For technical reasons, not all languages may be listed below. However, everything from \"{}\" commits is listed, so the list is likely complete.",
                platform.localization_change_commit_message()
            ),
            Complete => String::new(),
        }
    }
}
