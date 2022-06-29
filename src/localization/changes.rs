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
    pub old_tag: Tag,
    pub new_tag: Tag,
    pub complete: bool,
    pub changes: Rc<Vec<LocalizationChange<'a>>>,
}

impl<'a> LocalizationChanges<'a> {
    /// Note: assumes `if_incomplete_combine_with` is sorted.
    pub fn from_comparison(
        platform: &'a Platform,
        old_tag: Tag,
        new_tag: Tag,
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

        console_log!("after combining changes.len() = {:?}", changes.len());

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
                    "\n**Note:** These changes may not include all languages (GitHub API likely did not return all files). {}",
                    self.full_comparison_notice()
                )
            )
        };

        write!(
            f,
            "### {}{} changes compared to {}:{}{}",
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
