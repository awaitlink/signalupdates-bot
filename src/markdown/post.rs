use std::collections::HashMap;

use anyhow::bail;
use strum::IntoEnumIterator;

use super::{Commit, CommitStatus};
use crate::{
    discourse::{self, PostingOutcome},
    github::Tag,
    localization::{LocalizationChangeCollection, LocalizationChangeRenderMode},
    platform::Platform,
};

#[derive(Debug)]
pub struct Post<'a> {
    platform: Platform,
    old_tag: &'a Tag,
    new_tag: &'a Tag,
    commits: Vec<Commit<'a>>,
    unfiltered_commits_len: usize,
    localization_change_collection: LocalizationChangeCollection<'a>,
}

impl<'a> Post<'a> {
    pub fn new(
        platform: Platform,
        old_tag: &'a Tag,
        new_tag: &'a Tag,
        commits: Vec<Commit<'a>>,
        unfiltered_commits_len: usize,
        localization_change_collection: LocalizationChangeCollection<'a>,
    ) -> Self {
        assert!(commits.len() <= unfiltered_commits_len);

        Self {
            platform,
            old_tag,
            new_tag,
            commits,
            unfiltered_commits_len,
            localization_change_collection,
        }
    }

    fn commits_markdown(&self) -> String {
        let mut map = HashMap::new();

        for commit in self.commits.iter() {
            if let Some(sha) = commit.reverted_commit_sha() {
                map.insert(sha, commit.sha());
            }
        }

        let reverse_map: HashMap<_, _> = map
            .iter()
            .map(|(reverted, reverted_by)| (reverted_by, reverted))
            .collect();

        let commit_numbers: HashMap<&str, usize> =
            self.commits.iter().map(Commit::sha).zip(1..).collect();

        self.commits
            .iter()
            .zip(1..)
            .map(|(commit, number)| {
                commit.markdown_text(
                    number,
                    match (
                        map.get(commit.sha())
                            .and_then(|sha| commit_numbers.get(sha) /* there should always be a commit number for this sha, but leaving as is */),
                        reverse_map
                            .get(&commit.sha())
                            .and_then(|&sha| commit_numbers.get(sha)),
                    ) {
                        (Some(&reverted_by), Some(&reverted)) => CommitStatus::Both {
                            reverts: reverted,
                            is_reverted_by: reverted_by,
                        },
                        (Some(&reverted_by), None) => CommitStatus::IsRevertedBy(reverted_by),
                        (None, Some(&reverted)) => CommitStatus::Reverts(reverted),
                        (None, None) => CommitStatus::Normal,
                    },
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn markdown_text(&self, commits_markdown: &str, mode: LocalizationChangeRenderMode) -> String {
        let old_version = &self.old_tag.exact_version_string();
        let new_version = &self.new_tag.exact_version_string();

        let platform = self.platform;
        let availability_notice = platform.availability_notice();
        let comparison_url =
            platform.github_comparison_url(&self.old_tag.name, &self.new_tag.name, None);

        let commits_count = self.commits.len();
        let (commits_prefix, commits_postfix) = match commits_count {
            0..=20 => ("", ""),
            _ => ("[details=\"Show commits\"]\n", "\n[/details]"),
        };

        let commits_word_suffix = if commits_count == 1 { "" } else { "s" };

        let localization_changes_string = self.localization_change_collection.to_string(mode);

        let difference = self.unfiltered_commits_len - self.commits.len();
        let filtered_notice = if difference != 0 {
            let suffix = if difference == 1 { "" } else { "s" };
            format!(" (+ {} commit{suffix} omitted)", difference)
        } else {
            String::new()
        };

        format!(
            "## New Version: {new_version}{availability_notice}
[quote]
{commits_count} new commit{commits_word_suffix} since {old_version}{filtered_notice}:
{commits_prefix}{commits_markdown}{commits_postfix}
---
Gathered from [signalapp/Signal-{platform}]({comparison_url})
[/quote]
{localization_changes_string}"
        )
    }

    pub async fn post(
        &self,
        is_dry_run: bool,
        api_key: &str,
        topic_id: u64,
        reply_to_post_number: Option<u64>,
    ) -> anyhow::Result<PostingOutcome> {
        let commits_markdown = self.commits_markdown();

        let mut post_markdown: Option<String> = None;

        for mode in LocalizationChangeRenderMode::iter() {
            tracing::debug!("trying localization change collection render mode = {mode:?}");

            let text = self.markdown_text(&commits_markdown, mode);
            tracing::debug!("text.len() = {}", text.len());

            if text.len() > 32_000 {
                tracing::warn!("text is likely too long to post");
                post_markdown = None;
            } else {
                post_markdown = Some(text);
                break;
            }
        }

        match &post_markdown {
            Some(markdown_text) => {
                if !is_dry_run {
                    discourse::post(markdown_text, api_key, topic_id, reply_to_post_number).await
                } else {
                    tracing::warn!("dry run; not posting to Discourse");
                    Ok(PostingOutcome::Posted {
                        number: reply_to_post_number.unwrap_or(0),
                    })
                }
            }
            None => bail!("could not make a post that fits within the allowed character count"),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;
    use test_case::test_case;

    use super::*;
    use crate::{
        localization::{Completeness, LocalizationChange, LocalizationChanges, StringsFileKind::*},
        platform::Platform::{self, *},
    };

    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef")
    ], 1, None, "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
1 new commit since 1.2.3:
- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take a few seconds before GitHub jumps to the file (try scrolling a bit if it doesn't).

#### 0 languages changed since 1.2.3:
*No localization changes found*

Localization changes for the release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: one commit")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef"),
        Commit::new(Android, "Bump version to 1.2.4", "abc123")
    ], 2, None, "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
2 new commits since 1.2.3:
- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Bump version to 1.2.4 [[2]](https://github.com/signalapp/Signal-Android/commit/abc123)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take a few seconds before GitHub jumps to the file (try scrolling a bit if it doesn't).

#### 0 languages changed since 1.2.3:
*No localization changes found*

Localization changes for the release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: two commits")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef"),
        Commit::new(Android, "Bump version to 1.2.4", "abc123")
    ], 3, None, "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
2 new commits since 1.2.3 (+ 1 commit omitted):
- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Bump version to 1.2.4 [[2]](https://github.com/signalapp/Signal-Android/commit/abc123)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take a few seconds before GitHub jumps to the file (try scrolling a bit if it doesn't).

#### 0 languages changed since 1.2.3:
*No localization changes found*

Localization changes for the release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: two commits, one omitted")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef"),
        Commit::new(Android, "Bump version to 1.2.4", "abc123")
    ], 4, None, "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
2 new commits since 1.2.3 (+ 2 commits omitted):
- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Bump version to 1.2.4 [[2]](https://github.com/signalapp/Signal-Android/commit/abc123)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take a few seconds before GitHub jumps to the file (try scrolling a bit if it doesn't).

#### 0 languages changed since 1.2.3:
*No localization changes found*

Localization changes for the release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: two commits, two omitted")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abc111"),
        Commit::new(Android, "Revert \"Test commit.\".\nThis reverts commit abc111.", "abc222"),
        Commit::new(Android, "Revert \"Revert \"Test commit.\".\".\nThis reverts commit abc222.", "abc333"),
        Commit::new(Android, "Revert \"Test commit 0.\".\nThis reverts commit abc000.", "abc444"),
        Commit::new(Android, "Test commit 2.", "abc555"),
    ], 5, None, "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
5 new commits since 1.2.3:
- <del>Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abc111)</del> (reverted by [2])

- <del>Revert &quot;Test commit.&quot;. [[2]](https://github.com/signalapp/Signal-Android/commit/abc222)</del> (reverts [1], reverted by [3])

- <ins>Revert &quot;Revert &quot;Test commit.&quot;.&quot;. [[3]](https://github.com/signalapp/Signal-Android/commit/abc333)</ins> (reverts [2])

- Revert &quot;Test commit 0.&quot;. [[4]](https://github.com/signalapp/Signal-Android/commit/abc444)

- Test commit 2. [[5]](https://github.com/signalapp/Signal-Android/commit/abc555)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take a few seconds before GitHub jumps to the file (try scrolling a bit if it doesn't).

#### 0 languages changed since 1.2.3:
*No localization changes found*

Localization changes for the release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: five commits with reverts")]
    #[test_case(Android, "v1.2.3", "v1.2.4",
    std::iter::repeat(Commit::new(Android, "Test commit.", "abcdef"))
        .take(20)
        .chain(vec![Commit::new(Android, "Bump version to 1.2.4", "abc123")].iter().cloned())
        .collect(),
    21, None, "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
21 new commits since 1.2.3:
[details=\"Show commits\"]
- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[2]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[3]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[4]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[5]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[6]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[7]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[8]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[9]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[10]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[11]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[12]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[13]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[14]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[15]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[16]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[17]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[18]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[19]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Test commit. [[20]](https://github.com/signalapp/Signal-Android/commit/abcdef)

- Bump version to 1.2.4 [[21]](https://github.com/signalapp/Signal-Android/commit/abc123)

[/details]
---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take a few seconds before GitHub jumps to the file (try scrolling a bit if it doesn't).

#### 0 languages changed since 1.2.3:
*No localization changes found*

Localization changes for the release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: twenty one commits")]
    #[test_case(Desktop, "v1.2.3-beta.1", "v1.2.3-beta.2", vec![
        Commit::new(Desktop, "Test commit.", "abcdef")
    ], 1, None, "## New Version: 1.2.3-beta.2
[quote]
1 new commit since 1.2.3-beta.1:
- Test commit. [[1]](https://github.com/signalapp/Signal-Desktop/commit/abcdef)

---
Gathered from [signalapp/Signal-Desktop](https://github.com/signalapp/Signal-Desktop/compare/v1.2.3-beta.1...v1.2.3-beta.2)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take a few seconds before GitHub jumps to the file (try scrolling a bit if it doesn't).

#### 0 languages changed since 1.2.3-beta.1:
*No localization changes found*

Localization changes for the release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Desktop: one commit")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef")
    ], 1, Some(Completeness::Complete), "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
1 new commit since 1.2.3:
- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take a few seconds before GitHub jumps to the file (try scrolling a bit if it doesn't).

#### 2 languages changed since 1.2.3:
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en-US`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-33a220e7f3b2e559ebec12cbf273da0c135bfade5a547e41e2bb5a66d75a01d2)

#### 3 languages changed since 1.1.5:
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en-CA`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-11b72e6873b6a42a2a1b92999e4225d7ff2921e323baa5e7b31fdc49471d9724)
- [English (`en-US`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-33a220e7f3b2e559ebec12cbf273da0c135bfade5a547e41e2bb5a66d75a01d2)
[/quote]
[/details]"; "Android: one commit with localization changes")]
    fn post_markdown(
        platform: Platform,
        old_tag: &str,
        new_tag: &str,
        commits: Vec<Commit>,
        unfiltered_commits_len: usize,
        localization_change_collection: Option<Completeness>,
        result: &str,
    ) {
        let older_tag = Tag::new("v1.1.5");
        let old_tag = Tag::new(old_tag);
        let new_tag = Tag::new(new_tag);

        let localization_change_collection = match localization_change_collection {
            Some(completeness) => LocalizationChangeCollection {
                build_changes: LocalizationChanges {
                    platform: Android,
                    old_tag: &old_tag,
                    new_tag: &new_tag,
                    completeness: Completeness::Complete,
                    unsorted_changes: LocalizationChange::unsorted_changes(vec![
                        LocalizationChange::test_change("en", vec![Main]),
                        LocalizationChange::test_change("en-US", vec![Main]),
                    ]),
                },
                release_changes: Some(LocalizationChanges {
                    platform: Android,
                    old_tag: &older_tag,
                    new_tag: &new_tag,
                    completeness,
                    unsorted_changes: LocalizationChange::unsorted_changes(vec![
                        LocalizationChange::test_change("en", vec![Main]),
                        LocalizationChange::test_change("en-US", vec![Main]),
                        LocalizationChange::test_change("en-CA", vec![Main]),
                    ]),
                }),
            },
            None => LocalizationChangeCollection {
                build_changes: LocalizationChanges {
                    platform,
                    old_tag: &old_tag,
                    new_tag: &new_tag,
                    completeness: Completeness::Complete,
                    unsorted_changes: LocalizationChange::unsorted_changes(vec![]),
                },
                release_changes: None,
            },
        };

        let post = Post::new(
            platform,
            &old_tag,
            &new_tag,
            commits,
            unfiltered_commits_len,
            localization_change_collection,
        );

        assert_str_eq!(
            post.markdown_text(&post.commits_markdown(), LocalizationChangeRenderMode::Full),
            result
        );
    }
}
