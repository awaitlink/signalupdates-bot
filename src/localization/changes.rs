use std::fmt;

use worker::console_log;

use crate::{
    localization::{
        Completeness::{self, *},
        LocalizationChange,
    },
    platform::Platform,
    types::github::{Comparison, Tag},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationChanges<'a> {
    pub platform: Platform,
    pub old_tag: &'a Tag,
    pub new_tag: &'a Tag,
    pub completeness: Completeness,
    pub changes: Vec<LocalizationChange>,
}

impl<'a> LocalizationChanges<'a> {
    /// Note: assumes `comparison.files` is not `None` and doesn't contain duplicates.
    pub fn from_comparison(
        platform: Platform,
        old_tag: &'a Tag,
        new_tag: &'a Tag,
        comparison: &'a Comparison,
    ) -> LocalizationChanges<'a> {
        let complete = comparison.are_files_likely_complete().unwrap();
        console_log!("complete = {}", complete);

        let mut changes =
            LocalizationChange::unsorted_changes_from_files(platform, &comparison.files);

        changes.sort_unstable();

        console_log!("changes.len() = {:?}", changes.len());

        Self {
            platform,
            old_tag,
            new_tag,
            completeness: if complete { Complete } else { Incomplete },
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
            .map(|change| change.string(self.platform, self.old_tag, self.new_tag))
            .collect::<Vec<_>>()
            .join("\n- ")
    }
}

impl fmt::Display for LocalizationChanges<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (at_least, warning) = match self.completeness {
            Incomplete | LikelyComplete => (
                "At least ",
                format!(
                    "\n{} {}",
                    self.completeness.warning_text(),
                    self.full_comparison_notice()
                ),
            ),
            Complete => ("", String::new()),
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
            "#### {at_least}{changes_len} languages changed since {old_version}:{warning}{prefix}{changes}{suffix}"
        )
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;
    use test_case::test_case;

    use super::*;
    use crate::{
        localization::{LocalizationChange, LocalizationChanges},
        platform::Platform::{self, *},
    };

    #[test_case(Android, "v1.2.3", "v1.2.4", Complete, vec![
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android()
    ], "#### 2 languages changed since 1.2.3:
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)"; "Android: 2 languages changed, complete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", Incomplete, vec![
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android(),
    ], "#### At least 3 languages changed since 1.1.5:
:warning: For technical reasons, not all languages may be listed below. You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)"; "Android: 3 languages changed, incomplete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", LikelyComplete, vec![
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android(),
    ], "#### At least 3 languages changed since 1.1.5:
For technical reasons, not all languages may be listed below. However, everything from \"Updated language translations\" and similar commits is listed, so the list is likely complete. You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)"; "Android: 3 languages changed, likely complete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", Incomplete, std::iter::repeat(
        LocalizationChange::default_for_android()
    ).take(21).collect(), "#### At least 21 languages changed since 1.1.5:
:warning: For technical reasons, not all languages may be listed below. You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
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
[/details]"; "Android: 21 languages changed, incomplete")]
    fn to_string(
        platform: Platform,
        old_tag: &str,
        new_tag: &str,
        completeness: Completeness,
        changes: Vec<LocalizationChange>,
        result: &str,
    ) {
        let old_tag = Tag::new(old_tag);
        let new_tag = Tag::new(new_tag);

        let changes = LocalizationChanges {
            platform,
            old_tag: &old_tag,
            new_tag: &new_tag,
            completeness,
            changes,
        };

        assert_str_eq!(changes.to_string(), result);
    }
}
