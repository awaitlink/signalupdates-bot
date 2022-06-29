use std::borrow::Cow;

use lazy_static::lazy_static;
use regex::Regex;

use crate::{platform::Platform, types::github};

#[derive(Debug, Clone)]
pub struct Commit<'a> {
    platform: Platform,
    full_message: &'a str,
    sha: &'a str,
}

impl<'a> Commit<'a> {
    pub fn new(platform: Platform, full_message: &'a str, sha: &'a str) -> Self {
        Self {
            platform,
            full_message,
            sha,
        }
    }

    pub fn from_github_commit(platform: Platform, github_commit: &'a github::Commit) -> Self {
        Self::new(platform, &github_commit.commit.message, &github_commit.sha)
    }

    pub fn markdown_text(&self, number: usize) -> String {
        lazy_static! {
            static ref MENTION_REGEX: Regex = Regex::new(r"@([a-zA-Z0-9_-]+)").unwrap();
        }

        let message_lines: Vec<Cow<str>> = self
            .full_message
            .split('\n')
            .filter(|line| {
                !line.contains("Co-Authored-By") && !line.contains("This reverts commit")
            })
            .map(|line| MENTION_REGEX.replace_all(line, "`@$1`"))
            .collect();

        let message = match message_lines.get(0) {
            Some(line) => line,
            None => "*Empty commit message*",
        };

        let commit_url = self.platform.github_commit_url(self.sha);

        let main_content = format!("- {message} [[{number}]]({commit_url})\n");

        let details = match message_lines.len() {
            2.. => format!("\n    {}", message_lines[1..].join("\n    ")),
            _ => String::from(""),
        };

        main_content + &details
    }
}
#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;
    use crate::platform::Platform::{self, *};

    #[test_case(Android, "Test commit.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "Android: one line")]
    #[test_case(Android, "Test commit.\nAnother line.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.".to_string(); "Android: two lines")]
    #[test_case(Android, "Test commit.\nAnother line.\nAnd another line.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.\n    And another line.".to_string(); "Android: three lines")]
    #[test_case(Android, "Test commit.\nCo-Authored-By: user", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "Android: Co-Authored-By is removed")]
    #[test_case(Android, "Revert \"Test commit\".\nThis reverts commit fedcba.", "abcdef" => "- Revert \"Test commit\". [[1]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n".to_string(); "Android: This reverts commit is removed")]
    #[test_case(Desktop, "Test commit.", "abcdef" => "- Test commit. [[1]](https://github.com/signalapp/Signal-Desktop/commit/abcdef)\n".to_string(); "Desktop: one line")]
    #[test_case(Desktop, "Test commit. Test @mention!\nTest@mention2.", "abcdef" => "- Test commit. Test `@mention`! [[1]](https://github.com/signalapp/Signal-Desktop/commit/abcdef)\n\n    Test`@mention2`.".to_string(); "Desktop: two lines with mention")]
    fn commit_markdown(platform: Platform, full_message: &str, sha: &str) -> String {
        Commit::new(platform, full_message, sha).markdown_text(1)
    }
}
