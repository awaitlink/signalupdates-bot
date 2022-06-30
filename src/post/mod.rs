use std::collections::HashMap;

use anyhow::{bail, Context};
use serde_json::json;
use strum::IntoEnumIterator;
use worker::{console_error, console_log, console_warn, Method, Url};

use crate::{
    localization::{LocalizationChangeCollection, RenderMode},
    platform::Platform,
    types::{self, github::Tag},
    utils,
};

mod commit;

pub use commit::Commit;
use commit::CommitStatus;

#[derive(Debug)]
pub struct Post<'a> {
    platform: Platform,
    old_tag: &'a Tag,
    new_tag: &'a Tag,
    commits: Vec<Commit<'a>>,
    localization_change_collection: LocalizationChangeCollection<'a>,
}

impl<'a> Post<'a> {
    pub fn new(
        platform: Platform,
        old_tag: &'a Tag,
        new_tag: &'a Tag,
        commits: Vec<Commit<'a>>,
        localization_change_collection: LocalizationChangeCollection<'a>,
    ) -> Self {
        Self {
            platform,
            old_tag,
            new_tag,
            commits,
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

    fn markdown_text(&self, commits_markdown: &str, mode: RenderMode) -> String {
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

        format!(
            "## New Version: {new_version}{availability_notice}
[quote]
{commits_count} new commit{commits_word_suffix} since {old_version}:
{commits_prefix}{commits_markdown}{commits_postfix}
---
Gathered from [signalapp/Signal-{platform}]({comparison_url})
[/quote]
{localization_changes_string}"
        )
    }

    pub async fn post(
        &self,
        api_key: &str,
        topic_id: u64,
        reply_to_post_number: Option<u64>,
    ) -> anyhow::Result<u64> {
        let commits_markdown = self.commits_markdown();

        let mut post_markdown: Option<String> = None;

        for mode in RenderMode::iter() {
            console_log!("trying localization change collection render mode = {mode:?}");

            let text = self.markdown_text(&commits_markdown, mode);
            console_log!("text.len() = {}", text.len());

            if text.len() > 32_000 {
                console_warn!("text is likely too long to post");
                post_markdown = None;
            } else {
                post_markdown = Some(text);
                break;
            }
        }

        if post_markdown.is_none() {
            bail!("could not make a post that fits within the allowed character count")
        }

        let body = json!({
            "topic_id": topic_id,
            "reply_to_post_number": reply_to_post_number,
            "raw": post_markdown,
        });

        let url = Url::parse("https://community.signalusers.org/posts.json")
            .context("could not parse URL")?;
        let request = utils::create_request(url, Method::Post, Some(body), Some(api_key))?;

        let api_response: types::discourse::PostApiResponse =
            utils::get_json_from_request(request).await?;

        match api_response.post_number {
            Some(number) => Ok(number),
            None => {
                console_error!("api_response = {:?}", api_response);
                bail!(
                    "discourse API response did not include the post number, posting likely failed"
                )
            }
        }
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

    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef")
    ], None, "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
1 new commit since 1.2.3:
- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.

#### 0 changes since 1.2.3:
*No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: one commit")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef"),
        Commit::new(Android, "Bump version to 1.2.4", "abc123")
    ], None, "## New Version: 1.2.4
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
Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.

#### 0 changes since 1.2.3:
*No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: two commits")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abc111"),
        Commit::new(Android, "Revert \"Test commit.\".\nThis reverts commit abc111.", "abc222"),
        Commit::new(Android, "Revert \"Revert \"Test commit.\".\".\nThis reverts commit abc222.", "abc333"),
        Commit::new(Android, "Revert \"Test commit 0.\".\nThis reverts commit abc000.", "abc444"),
        Commit::new(Android, "Test commit 2.", "abc555"),
    ], None, "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
5 new commits since 1.2.3:
- <del>Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abc111)</del> (reverted by [2])

- <del>Revert \"Test commit.\". [[2]](https://github.com/signalapp/Signal-Android/commit/abc222)</del> (reverts [1], reverted by [3])

- <ins>Revert \"Revert \"Test commit.\".\". [[3]](https://github.com/signalapp/Signal-Android/commit/abc333)</ins> (reverts [2])

- Revert \"Test commit 0.\". [[4]](https://github.com/signalapp/Signal-Android/commit/abc444)

- Test commit 2. [[5]](https://github.com/signalapp/Signal-Android/commit/abc555)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.

#### 0 changes since 1.2.3:
*No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: five commits with reverts")]
    #[test_case(Android, "v1.2.3", "v1.2.4",
    std::iter::repeat(Commit::new(Android, "Test commit.", "abcdef"))
        .take(20)
        .chain(vec![Commit::new(Android, "Bump version to 1.2.4", "abc123")].iter().cloned())
        .collect(),
    None, "## New Version: 1.2.4
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
Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.

#### 0 changes since 1.2.3:
*No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Android: twenty one commits")]
    #[test_case(Desktop, "v1.2.3-beta.1", "v1.2.3-beta.2", vec![
        Commit::new(Desktop, "Test commit.", "abcdef")
    ], None, "## New Version: 1.2.3-beta.2
[quote]
1 new commit since 1.2.3-beta.1:
- Test commit. [[1]](https://github.com/signalapp/Signal-Desktop/commit/abcdef)

---
Gathered from [signalapp/Signal-Desktop](https://github.com/signalapp/Signal-Desktop/compare/v1.2.3-beta.1...v1.2.3-beta.2)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.

#### 0 changes since 1.2.3-beta.1:
*No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]"; "Desktop: one commit")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef")
    ], Some(true), "## New Version: 1.2.4
(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)
[quote]
1 new commit since 1.2.3:
- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)

---
Gathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.

#### 2 changes since 1.2.3:
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)

#### 3 changes since 1.1.5:
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
[/quote]
[/details]"; "Android: one commit with localization changes")]
    fn post_markdown(
        platform: Platform,
        old_tag: &str,
        new_tag: &str,
        commits: Vec<Commit>,
        localization_change_collection: Option<bool>,
        result: &str,
    ) {
        let older_tag = Tag::new("v1.1.5");
        let old_tag = Tag::new(old_tag);
        let new_tag = Tag::new(new_tag);

        let localization_change_collection = match localization_change_collection {
            Some(are_release_changes_complete) => LocalizationChangeCollection {
                build_changes: LocalizationChanges {
                    platform: Android,
                    old_tag: &old_tag,
                    new_tag: &new_tag,
                    complete: true,
                    changes: vec![
                        LocalizationChange::default_for_android(),
                        LocalizationChange::default_for_android(),
                    ],
                },
                release_changes: Some(LocalizationChanges {
                    platform: Android,
                    old_tag: &older_tag,
                    new_tag: &new_tag,
                    complete: are_release_changes_complete,
                    changes: vec![
                        LocalizationChange::default_for_android(),
                        LocalizationChange::default_for_android(),
                        LocalizationChange::default_for_android(),
                    ],
                }),
            },
            None => LocalizationChangeCollection {
                build_changes: LocalizationChanges {
                    platform,
                    old_tag: &old_tag,
                    new_tag: &new_tag,
                    complete: true,
                    changes: vec![],
                },
                release_changes: None,
            },
        };

        let post = Post::new(
            platform,
            &old_tag,
            &new_tag,
            commits,
            localization_change_collection,
        );

        assert_str_eq!(
            post.markdown_text(&post.commits_markdown(), RenderMode::Full),
            result
        );
    }
}
