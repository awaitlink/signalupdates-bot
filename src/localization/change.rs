use super::Language;
use crate::{platform::Platform, types::github::Comparison};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalizationChange {
    pub language: Language,
    pub filename: String,
}

impl LocalizationChange {
    pub fn changes_from_comparison(
        platform: Platform,
        comparison: &Comparison,
    ) -> Vec<LocalizationChange> {
        let mut changes = comparison
            .files
            .clone()
            .unwrap()
            .iter()
            .filter_map(|file| platform.localization_change(&file.filename))
            .collect::<Vec<_>>();

        changes.sort_unstable();

        changes
    }
}
