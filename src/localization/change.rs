use super::Language;
use crate::{platform::Platform, types::github::Comparison};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalizationChange<'a> {
    pub language: Language,
    pub filename: &'a str,
}

impl<'a> LocalizationChange<'a> {
    pub fn changes_from_comparison(
        platform: &'a Platform,
        comparison: &'a Comparison,
    ) -> Vec<LocalizationChange<'a>> {
        let mut changes = comparison
            .files
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(move |file| platform.localization_change(&file.filename))
            .collect::<Vec<_>>();

        changes.sort_unstable();

        changes
    }
}
