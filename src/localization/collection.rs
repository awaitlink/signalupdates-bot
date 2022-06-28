use strum_macros::EnumIter;

use super::LocalizationChange;
use crate::{platform::Platform, types::github::Tag};

#[derive(Debug, EnumIter, Clone, Copy)]
pub enum RenderMode {
    Full,
    WithoutRelease,
    Nothing,
}

use RenderMode::*;

#[derive(Debug)]
pub struct LocalizationChangeCollection<'a> {
    pub build_changes: Vec<LocalizationChange<'a>>,
    pub release_changes: Option<(Tag, Vec<LocalizationChange<'a>>)>,
    pub are_release_changes_complete: bool,
}

impl<'a> LocalizationChangeCollection<'a> {
    pub fn to_string(
        &self,
        platform: Platform,
        old_tag: &Tag,
        new_tag: &Tag,
        mode: RenderMode,
    ) -> String {
        let changes = match (mode, &self.release_changes) {
            (Full, Some((tag, changes))) => vec![
                (old_tag, self.build_changes.clone()),
                (tag, changes.clone()),
            ],
            (Full, None) | (WithoutRelease, _) => {
                vec![(old_tag, self.build_changes.clone())]
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

        let none_fit_notice = "Sorry, no localization changes fit in the post character limit.";
        let same_notice = "Localization changes for the whole release are the same, as this is the first build of the release.";
        let build_diff_notice = || {
            format!(
                "You can view the full comparison to {} by following the \"gathered from\" link above.",
                old_tag.exact_version_string()
            )
        };
        let release_diff_notice = |tag: &Tag| {
            format!(
                "You can view the full comparison to {} so far [on GitHub]({}).",
                tag.exact_version_string(),
                platform.github_comparison_url(&tag.name, &new_tag.name, None)
            )
        };

        let notice = match (mode, &self.release_changes) {
            (Full, Some((tag, _))) => if self.are_release_changes_complete {
                String::from("")
            } else {
                format!(
                    "\n\n**Note:** Localization changes for the whole release may not include all languages (GitHub API likely did not return all files). {}",
                    release_diff_notice(tag)
                )
            },
            (Full | WithoutRelease, None) => format!("\n\n{}", same_notice),
            (WithoutRelease, Some((tag, _))) => format!(
                "\n\nSorry, localization changes for the whole release did not fit in the post character limit. {}",
                release_diff_notice(tag)
            ),
            (Nothing, Some((tag, _))) => format!("\n\n{} {} {}", none_fit_notice, build_diff_notice(), release_diff_notice(tag)),
            (Nothing, None) => format!("\n\n{} {} {}", none_fit_notice, build_diff_notice(), same_notice)
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
        old_tag: &Tag,
        new_tag: &Tag,
        changes: &[LocalizationChange],
    ) -> String {
        format!(
            "Compared to {}:{}",
            old_tag.exact_version_string(),
            match changes.len() {
                1.. => format!(
                    "\n- {}",
                    Self::language_links(platform, old_tag, new_tag, changes)
                ),
                _ => String::from(" *No localization changes found*"),
            }
        )
    }

    fn language_links(
        platform: Platform,
        old_tag: &Tag,
        new_tag: &Tag,
        changes: &[LocalizationChange],
    ) -> String {
        changes
            .iter()
            .map(|change| {
                format!(
                    "[{}]({})",
                    change.language,
                    platform.github_comparison_url(
                        &old_tag.name,
                        &new_tag.name,
                        Some(change.filename)
                    )
                )
            })
            .collect::<Vec<_>>()
            .join("\n- ")
    }
}
