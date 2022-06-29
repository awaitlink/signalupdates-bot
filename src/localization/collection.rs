use strum_macros::EnumIter;

use super::LocalizationChanges;

#[derive(Debug, EnumIter, Clone, Copy)]
pub enum RenderMode {
    Full,
    WithoutRelease,
    Nothing,
}

use RenderMode::*;

#[derive(Debug)]
pub struct LocalizationChangeCollection<'a> {
    pub build_changes: LocalizationChanges<'a>,
    pub release_changes: Option<LocalizationChanges<'a>>,
}

impl<'a> LocalizationChangeCollection<'a> {
    pub fn to_string(&self, mode: RenderMode) -> String {
        let changes = match (mode, &self.release_changes) {
            (Full, Some(changes)) => vec![&self.build_changes, changes],
            (Full, None) | (WithoutRelease, _) => {
                vec![&self.build_changes]
            }
            (Nothing, _) => vec![],
        }
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n\n");

        let usage_instructions = match mode {
            Nothing => "",
            _ => "Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.\n\n",
        };

        let none_fit_notice = "Sorry, no localization changes fit in the post character limit.";
        let same_notice = "Localization changes for the whole release are the same, as this is the first build of the release.";

        let notice = match (mode, &self.release_changes) {
            (Full, Some(_)) => String::from(""),
            (Full | WithoutRelease, None) => format!("\n\n{}", same_notice),
            (WithoutRelease, Some(release_changes)) => format!(
                "\n\nSorry, localization changes for the whole release did not fit in the post character limit. {}",
                release_changes.full_comparison_notice()
            ),
            (Nothing, Some(release_changes)) => format!(
                "\n\n{} {} {}",
                none_fit_notice,
                self.build_changes.full_comparison_notice(),
                release_changes.full_comparison_notice()
            ),
            (Nothing, None) => format!(
                "\n\n{} {} {}",
                none_fit_notice,
                self.build_changes.full_comparison_notice(),
                same_notice
            )
        };

        format!(
            "[details=\"Localization changes\"]
[quote]
{usage_instructions}{changes}{notice}
[/quote]
[/details]"
        )
    }
}
