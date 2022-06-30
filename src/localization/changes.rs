use std::fmt;

use worker::console_log;

use super::LocalizationChange;
use crate::{
    platform::Platform,
    types::github::{Comparison, Tag},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationChanges<'a> {
    pub platform: Platform,
    pub old_tag: &'a Tag,
    pub new_tag: &'a Tag,
    pub complete: bool,
    pub changes: Vec<LocalizationChange>,
}

impl<'a> LocalizationChanges<'a> {
    /// Note: assumes `comparison.files` is not `None` and doesn't contain duplicates.
    pub fn from_comparison(
        platform: &'a Platform,
        old_tag: &'a Tag,
        new_tag: &'a Tag,
        comparison: &'a Comparison,
    ) -> LocalizationChanges<'a> {
        let complete = comparison.are_files_likely_complete().unwrap();
        console_log!("complete = {}", complete);

        let mut changes = comparison
            .files
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(move |file| platform.localization_change(&file.filename))
            .collect::<Vec<_>>();

        changes.sort_unstable();

        console_log!("changes.len() = {:?}", changes.len());

        Self {
            platform: *platform,
            old_tag,
            new_tag,
            complete,
            changes,
        }
    }

    pub fn full_comparison_notice(&self) -> String {
        format!(
            "You can view the full comparison to {} so far [here]({}).",
            self.old_tag.exact_version_string(),
            self.platform
                .github_comparison_url(&self.old_tag.name, &self.new_tag.name, None)
        )
    }

    fn language_links(&self) -> String {
        self.changes
            .iter()
            .map(|change| {
                format!(
                    "[{}]({})",
                    change.language,
                    self.platform.github_comparison_url(
                        &self.old_tag.name,
                        &self.new_tag.name,
                        Some(&change.filename)
                    )
                )
            })
            .collect::<Vec<_>>()
            .join("\n- ")
    }
}

impl fmt::Display for LocalizationChanges<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (at_least, warning) = if self.complete {
            ("", String::new())
        } else {
            (
                "At least ",
                format!(
                    "\n:warning: These changes may not include all languages (GitHub API likely did not return all files). {}",
                    self.full_comparison_notice()
                )
            )
        };

        let old_version = self.old_tag.exact_version_string();
        let changes_len = self.changes.len();

        let (prefix, suffix) = match changes_len {
            0..=20 => ("", ""),
            _ => ("\n[details=\"Show changes\"]", "\n[/details]"),
        };

        let changes = match changes_len {
            1.. => format!("\n- {}", self.language_links()),
            _ => String::from("\n*No localization changes found*"),
        };

        write!(
            f,
            "#### {at_least}{changes_len} changes since {old_version}:{warning}{prefix}{changes}{suffix}"
        )
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;
    use crate::{
        localization::{LocalizationChange, LocalizationChanges},
        platform::Platform::{self, *},
    };

    #[test_case(Android, "v1.2.3", "v1.2.4", true, vec![
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android()
    ], "#### 2 changes since 1.2.3:
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)"; "Android: 2 changes, complete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", false, vec![
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android(),
    ], "#### At least 3 changes since 1.1.5:
:warning: These changes may not include all languages (GitHub API likely did not return all files). You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)"; "Android: 3 changes, incomplete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", false, std::iter::repeat(
        LocalizationChange::default_for_android()
    ).take(21).collect(), "#### At least 21 changes since 1.1.5:
:warning: These changes may not include all languages (GitHub API likely did not return all files). You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
[details=\"Show changes\"]
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
[/details]"; "Android: 21 changes, incomplete")]
    fn to_string(
        platform: Platform,
        old_tag: &str,
        new_tag: &str,
        complete: bool,
        changes: Vec<LocalizationChange>,
        result: &str,
    ) {
        let old_tag = Tag::new(old_tag);
        let new_tag = Tag::new(new_tag);

        let changes = LocalizationChanges {
            platform,
            old_tag: &old_tag,
            new_tag: &new_tag,
            complete,
            changes,
        };

        assert_eq!(changes.to_string(), result);
    }
}
