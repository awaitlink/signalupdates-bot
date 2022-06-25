use anyhow::{bail, Context};
use serde_json::json;
use strum::IntoEnumIterator;
use worker::{console_log, Method, Url};

use crate::{
    localization_change::{LocalizationChangeCollection, LocalizationChangeCollectionRenderMode},
    platform::Platform,
    types, utils,
};

#[derive(Debug)]
pub struct Post {
    platform: Platform,
    old_tag: String,
    new_tag: String,
    commits: Vec<Commit>,
    localization_change_collection: LocalizationChangeCollection,
}

impl Post {
    pub fn new(
        platform: Platform,
        old_tag: impl Into<String>,
        new_tag: impl Into<String>,
        commits: Vec<Commit>,
        localization_change_collection: LocalizationChangeCollection,
    ) -> Self {
        Self {
            platform,
            old_tag: old_tag.into(),
            new_tag: new_tag.into(),
            commits,
            localization_change_collection,
        }
    }

    pub fn markdown_text(&self, mode: LocalizationChangeCollectionRenderMode) -> String {
        let commits = self
            .commits
            .iter()
            .enumerate()
            .map(|(index, commit)| commit.markdown_text(index + 1))
            .collect::<Vec<_>>()
            .join("\n");

        let old_version = utils::exact_version_string_from_tag(&self.old_tag);
        let new_version = utils::exact_version_string_from_tag(&self.new_tag);

        let platform = self.platform;
        let availability_notice = platform.availability_notice();
        let comparison_url = platform.github_comparison_url(&self.old_tag, &self.new_tag, None);

        let commits_count = self.commits.len();
        let (commits_prefix, commits_postfix) = match commits_count {
            0..=20 => ("", ""),
            _ => ("[details=\"Show commits\"]\n", "\n[/details]"),
        };

        let commits_word_suffix = if commits_count == 1 { "" } else { "s" };

        let localization_changes_string = self.localization_change_collection.to_string(
            platform,
            &self.old_tag,
            &self.new_tag,
            mode,
        );

        format!(
            "## New Version: {new_version}{availability_notice}
[quote]
{commits_count} new commit{commits_word_suffix} since {old_version}:
{commits_prefix}{commits}{commits_postfix}
---
Gathered from [signalapp/Signal-{platform}]({comparison_url})
[/quote]
{localization_changes_string}"
        )
    }

    pub async fn post(
        &self,
        api_key: String,
        topic_id: u64,
        reply_to_post_number: Option<u64>,
    ) -> anyhow::Result<u64> {
        let mut markdown_text: Option<String> = None;

        for mode in LocalizationChangeCollectionRenderMode::iter() {
            console_log!("trying localization change collection render mode = {mode:?}");

            let text = self.markdown_text(mode);
            console_log!("text.len() = {}", text.len());

            if text.len() > 32_000 {
                console_log!("text is likely too long to post");
                markdown_text = None;
            } else {
                markdown_text = Some(text);
                break;
            }
        }

        if markdown_text.is_none() {
            bail!("could not make a post that fits within the allowed character count")
        }

        let body = json!({
            "topic_id": topic_id,
            "reply_to_post_number": reply_to_post_number,
            "raw": markdown_text,
        });

        let url = Url::parse("https://community.signalusers.org/posts.json")
            .context("could not parse URL")?;
        let request = utils::create_request(url, Method::Post, Some(body), Some(api_key))?;

        let api_response: types::discourse::PostApiResponse =
            utils::get_json_from_request(request).await?;

        match api_response.post_number {
            Some(number) => Ok(number),
            None => {
                console_log!("api_response = {:?}", api_response);
                bail!(
                    "discourse API response did not include the post number, posting likely failed"
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Commit {
    platform: Platform,
    message_lines: Vec<String>,
    sha: String,
}

impl Commit {
    pub fn new(
        platform: Platform,
        full_message: impl Into<String>,
        sha: impl Into<String>,
    ) -> Self {
        Self {
            platform,
            message_lines: full_message
                .into()
                .split('\n')
                .filter(|line| {
                    !line.contains("Co-Authored-By") && !line.contains("This reverts commit")
                })
                .map(String::from)
                .collect(),
            sha: sha.into(),
        }
    }

    pub fn markdown_text(&self, number: usize) -> String {
        let message = match self.message_lines.get(0) {
            Some(line) => line,
            None => "*Empty commit message*",
        };

        let commit_url = self.platform.github_commit_url(&self.sha);

        let main_content = format!("- {message} [[{number}]]({commit_url})\n");

        let details = match self.message_lines.len() {
            2.. => format!("\n    {}", self.message_lines[1..].join("\n    ")),
            _ => String::from(""),
        };

        main_content + &details
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localization_change::LocalizationChange;
    use test_case::test_case;
    use Platform::*;

    #[test_case(Android, "Test commit.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "Android: one line")]
    #[test_case(Android, "Test commit.\nAnother line.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.".to_string(); "Android: two lines")]
    #[test_case(Android, "Test commit.\nAnother line.\nAnd another line.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.\n    And another line.".to_string(); "Android: three lines")]
    #[test_case(Android, "Test commit.\nCo-Authored-By: user", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "Android: Co-Authored-By is removed")]
    #[test_case(Android, "Revert \"Test commit\".\nThis reverts commit fedcba.", "abcdef" => "- Revert \"Test commit\". [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "Android: This reverts commit is removed")]
    #[test_case(Desktop, "Test commit.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Desktop/commit/abcdef)\n".to_string(); "Desktop: one line")]
    fn commit_markdown(
        platform: Platform,
        full_message: impl Into<String>,
        sha: impl Into<String>,
    ) -> String {
        Commit::new(platform, full_message, sha).markdown_text(1)
    }

    fn default_android_localization_change() -> LocalizationChange {
        LocalizationChange {
            language: Default::default(),
            filename: crate::platform::ANDROID_DEFAULT_STRINGS_FILENAME.to_string(),
        }
    }

    fn empty_localization_change_collection() -> LocalizationChangeCollection {
        LocalizationChangeCollection {
            build_localization_changes: vec![],
            release_localization_changes: None,
            is_release_complete: true,
        }
    }

    fn simple_localization_change_collection() -> LocalizationChangeCollection {
        LocalizationChangeCollection {
            build_localization_changes: vec![
                default_android_localization_change(),
                default_android_localization_change(),
            ],
            release_localization_changes: Some((
                String::from("v1.1.5"),
                vec![
                    default_android_localization_change(),
                    default_android_localization_change(),
                    default_android_localization_change(),
                ],
            )),
            is_release_complete: true,
        }
    }

    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef")
    ], empty_localization_change_collection() => "## New Version: 1.2.4
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

Compared to 1.2.3: *No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]".to_string(); "Android: one commit")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef"),
        Commit::new(Android, "Bump version to 1.2.4", "abc123")
    ], empty_localization_change_collection() => "## New Version: 1.2.4
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

Compared to 1.2.3: *No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]".to_string(); "Android: two commits")]
    #[test_case(Android, "v1.2.3", "v1.2.4",
    std::iter::repeat(Commit::new(Android, "Test commit.", "abcdef"))
        .take(20)
        .chain(vec![Commit::new(Android, "Bump version to 1.2.4", "abc123")].iter().cloned())
        .collect(),
    empty_localization_change_collection() => "## New Version: 1.2.4
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

Compared to 1.2.3: *No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]".to_string(); "Android: twenty one commits")]
    #[test_case(Desktop, "v1.2.3-beta.1", "v1.2.3-beta.2", vec![
        Commit::new(Desktop, "Test commit.", "abcdef")
    ], empty_localization_change_collection() => "## New Version: 1.2.3-beta.2
[quote]
1 new commit since 1.2.3-beta.1:
- Test commit. [[1]](https://github.com/signalapp/Signal-Desktop/commit/abcdef)

---
Gathered from [signalapp/Signal-Desktop](https://github.com/signalapp/Signal-Desktop/compare/v1.2.3-beta.1...v1.2.3-beta.2)
[/quote]
[details=\"Localization changes\"]
[quote]
Note: after clicking a link, it may take ~5-10s before GitHub jumps to the corresponding file.

Compared to 1.2.3-beta.1: *No localization changes found*

Localization changes for the whole release are the same, as this is the first build of the release.
[/quote]
[/details]".to_string(); "Desktop: one commit")]
    #[test_case(Android, "v1.2.3", "v1.2.4", vec![
        Commit::new(Android, "Test commit.", "abcdef")
    ], simple_localization_change_collection() => "## New Version: 1.2.4
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

Compared to 1.2.3: [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103) • [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)

Compared to 1.1.5: [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103) • [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103) • [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
[/quote]
[/details]".to_string(); "Android: one commit with localization changes")]
    fn post_markdown(
        platform: Platform,
        old_tag: impl Into<String>,
        new_tag: impl Into<String>,
        commits: Vec<Commit>,
        localization_change_collection: LocalizationChangeCollection,
    ) -> String {
        Post::new(
            platform,
            old_tag,
            new_tag,
            commits,
            localization_change_collection,
        )
        .markdown_text(LocalizationChangeCollectionRenderMode::Full)
    }
}
