use std::{fmt, rc::Rc};

use worker::{console_log, console_warn};

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
    pub changes: Rc<Vec<LocalizationChange<'a>>>,
}

impl<'a> LocalizationChanges<'a> {
    /// Note: assumes `if_incomplete_combine_with` is sorted.
    pub fn from_comparison(
        platform: &'a Platform,
        old_tag: &'a Tag,
        new_tag: &'a Tag,
        comparison: &'a Comparison,
        if_incomplete_combine_with: Option<Rc<Vec<LocalizationChange<'a>>>>,
    ) -> LocalizationChanges<'a> {
        let mut complete = true;

        let mut changes = comparison
            .files
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(move |file| platform.localization_change(&file.filename))
            .collect::<Vec<_>>();

        console_log!("changes.len() = {:?}", changes.len());

        // GitHub API only returns at most 300 files, despite
        // https://docs.github.com/en/rest/commits/commits#compare-two-commits
        // saying that it always returns all.
        let changes = if comparison.files.as_ref().unwrap().len() == 300 {
            console_warn!("`comparison` has 300 files, likely incomplete");
            complete = false;

            match if_incomplete_combine_with {
                Some(if_incomplete_combine_with) => {
                    if !changes.is_empty() {
                        console_log!("merging `changes` and `if_incomplete_combine_with`");

                        let mut combined = (*if_incomplete_combine_with).clone();
                        combined.append(&mut changes);
                        combined.dedup();
                        combined.sort_unstable();
                        Rc::new(combined)
                    } else {
                        console_log!("`changes` is empty, taking `if_incomplete_combine_with`");

                        Rc::clone(&if_incomplete_combine_with)
                    }
                }
                None => {
                    console_log!("no `if_incomplete_combine_with` specified, taking `changes`");
                    changes.sort_unstable();
                    Rc::new(changes)
                }
            }
        } else {
            console_log!("`comparison` appears to be complete");
            changes.sort_unstable();
            Rc::new(changes)
        };

        console_log!(
            "after (potentially) combining, changes.len() = {:?}",
            changes.len()
        );

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
                        Some(change.filename)
                    )
                )
            })
            .collect::<Vec<_>>()
            .join("\n- ")
    }
}

impl fmt::Display for LocalizationChanges<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let incomplete_notice = if self.complete {
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

        write!(
            f,
            "#### {}{} changes since {}:{}{}",
            incomplete_notice.0,
            self.changes.len(),
            self.old_tag.exact_version_string(),
            incomplete_notice.1,
            match self.changes.len() {
                1.. => format!("\n- {}", self.language_links()),
                _ => String::from("\n*No localization changes found*"),
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

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
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)"; "Android: 2 changes, complete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", false, vec![
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android(),
        LocalizationChange::default_for_android(),
    ], "#### At least 3 changes since 1.1.5:
:warning: These changes may not include all languages (GitHub API likely did not return all files). You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)"; "Android: 3 changes, incomplete")]
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
            changes: Rc::new(changes),
        };

        assert_eq!(changes.to_string(), result);
    }
}
