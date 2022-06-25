use strum_macros::EnumIter;

use crate::{language::Language, platform::Platform, utils};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationChange {
    pub language: Language,
    pub filename: String,
}

#[derive(Debug, EnumIter, Clone, Copy)]
pub enum LocalizationChangeCollectionRenderMode {
    Full,
    WithoutRelease,
    Nothing,
}
use LocalizationChangeCollectionRenderMode::*;

#[derive(Debug)]
pub struct LocalizationChangeCollection {
    pub build_localization_changes: Vec<LocalizationChange>,
    pub release_localization_changes: Option<(String, Vec<LocalizationChange>)>,
    pub is_release_complete: bool,
}

impl LocalizationChangeCollection {
    pub fn to_string(
        &self,
        platform: Platform,
        old_tag: &str,
        new_tag: &str,
        mode: LocalizationChangeCollectionRenderMode,
    ) -> String {
        let changes = match (mode, &self.release_localization_changes) {
            (Full, Some((tag, changes))) => vec![
                (old_tag, self.build_localization_changes.clone()),
                (tag, changes.clone()),
            ],
            (Full, None) | (WithoutRelease, _) => {
                vec![(old_tag, self.build_localization_changes.clone())]
            }
            (Nothing, _) => vec![],
        }
        .iter()
        .map(|(tag, changes)| {
            Self::string_for_localization_changes(platform, tag, new_tag, changes)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

        let usage_instructions = match mode {
            Nothing => "",
            _ => "Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.\n\n",
        };

        let notice = match (mode, &self.release_localization_changes) {
            (Full, Some(_)) => if self.is_release_complete { "" } else { "\n\n**Note:** Localization changes for the whole release may not include all languages (GitHub API likely did not return all files)." },
            (Full, None) => "\n\nLocalization changes for the whole release are the same, as this is the first build of the release.",
            (WithoutRelease, _) => "\n\nSorry, localization changes for the whole release did not fit in the post character limit.",
            (Nothing, _) => "Sorry, no localization changes fit in the post character limit.",
        };

        format!(
            "[details=\"Localization changes\"]
[quote]
{usage_instructions}{changes}{notice}
[/quote]
[/details]"
        )
    }

    fn string_for_localization_changes(
        platform: Platform,
        old_tag: &str,
        new_tag: &str,
        changes: &[LocalizationChange],
    ) -> String {
        format!(
            "Compared to {}: {}",
            utils::exact_version_string_from_tag(old_tag),
            match changes.len() {
                1.. => Self::language_links(platform, old_tag, new_tag, changes),
                _ => String::from("*No localization changes found*"),
            }
        )
    }

    fn language_links(
        platform: Platform,
        old_tag: &str,
        new_tag: &str,
        changes: &[LocalizationChange],
    ) -> String {
        changes
            .iter()
            .map(|change| {
                format!(
                    "[{}]({})",
                    change.language,
                    platform.github_comparison_url(old_tag, new_tag, Some(&change.filename))
                )
            })
            .collect::<Vec<_>>()
            .join(" • ")
    }
}
