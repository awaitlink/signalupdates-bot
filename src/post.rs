use anyhow::{anyhow, Context};
use serde_json::json;
use worker::{console_log, Method, Url};

use crate::{types, utils};

const POSTING_URL: &str = "https://community.signalusers.org/posts.json";
const FAD_TOPIC_URL: &str = "https://community.signalusers.org/t/17538";
const COMPARE_URL: &str = "https://github.com/signalapp/Signal-Android/compare";
const COMMIT_URL: &str = "https://github.com/signalapp/Signal-Android/commit";

#[derive(Debug)]
pub struct Post {
    previous_tag: String,
    new_tag: String,
    commits: Vec<Commit>,
}

impl Post {
    pub fn new(
        previous_tag: impl Into<String>,
        new_tag: impl Into<String>,
        commits: Vec<Commit>,
    ) -> Self {
        Self {
            previous_tag: previous_tag.into(),
            new_tag: new_tag.into(),
            commits,
        }
    }

    pub fn markdown_text(&self) -> String {
        let commits = self
            .commits
            .iter()
            .enumerate()
            .map(|(index, commit)| commit.markdown_text(index))
            .collect::<Vec<_>>()
            .join("\n");

        let previous_tag = &self.previous_tag;
        let previous_version = previous_tag.replace('v', "");

        let new_tag = &self.new_tag;
        let new_version = new_tag.replace('v', "");

        format!(
            "## :tada: - New Version: {new_version}
(Not Yet) Available via [Firebase App Distribution]({FAD_TOPIC_URL})

[quote]
All new commits since {previous_version}:

{commits}

---
Gathered from [signalapp/Signal-Android]({COMPARE_URL}/{previous_tag}...{new_tag})
[/quote]"
        )
    }

    pub async fn post(
        &self,
        api_key: String,
        topic_id: u64,
        reply_to_post_number: Option<u64>,
    ) -> anyhow::Result<u64> {
        let body = json!({
            "topic_id": topic_id,
            "reply_to_post_number": reply_to_post_number,
            "raw": self.markdown_text(),
        });

        let url = Url::parse(POSTING_URL).context("could not parse URL")?;
        let request = utils::create_request(url, Method::Post, Some(body), Some(api_key))?;

        let api_response: types::discourse::PostApiResponse =
            utils::get_json_from_request(request).await?;

        console_log!("api_response = {:?}", api_response);

        match api_response.post_number {
            Some(number) => Ok(number),
            None => Err(anyhow!(
                "discourse API response did not include the post number, posting likely failed"
            )),
        }
    }
}

#[derive(Debug)]
pub struct Commit {
    message_lines: Vec<String>,
    sha: String,
}

impl Commit {
    pub fn new(full_message: impl Into<String>, sha: impl Into<String>) -> Self {
        Self {
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

    pub fn markdown_text(&self, index: usize) -> String {
        let index = index + 1;

        let message = match self.message_lines.get(0) {
            Some(line) => line,
            None => "*Empty commit message*",
        };

        let sha = &self.sha;

        let main_content = format!("- {message} [[{index}]]({COMMIT_URL}/{sha})\n");

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
    use test_case::test_case;

    #[test_case("Test commit.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "one line")]
    #[test_case("Test commit.\nAnother line.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.".to_string(); "two lines")]
    #[test_case("Test commit.\nAnother line.\nAnd another line.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.\n    And another line.".to_string(); "three lines")]
    #[test_case("Test commit.\nCo-Authored-By: user", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "Co-Authored-By is removed")]
    #[test_case("Revert \"Test commit\".\nThis reverts commit fedcba.", "abcdef" => "- Revert \"Test commit\". [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "This reverts commit is removed")]
    fn commit_markdown(full_message: impl Into<String>, sha: impl Into<String>) -> String {
        Commit::new(full_message, sha).markdown_text(0)
    }

    #[test_case("v1.2.3", "v1.2.4", vec![
        Commit::new("Test commit.", "abcdef")
    ] => "## :tada: - New Version: 1.2.4\n(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)\n\n[quote]\nAll new commits since 1.2.3:\n\n- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n\n---\nGathered from [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android/compare/v1.2.3...v1.2.4)\n[/quote]".to_string(); "one commit")]
    fn post_markdown(
        previous_tag: impl Into<String>,
        new_tag: impl Into<String>,
        commits: Vec<Commit>,
    ) -> String {
        Post::new(previous_tag, new_tag, commits).markdown_text()
    }
}
