use std::borrow::Cow;

use lazy_static::lazy_static;
use regex::Regex;

use crate::{platform::Platform, types::github};

pub enum CommitStatus {
    Both {
        reverts: usize,
        is_reverted_by: usize,
    },
    IsRevertedBy(usize),
    Reverts(usize),
    Normal,
}

use CommitStatus::*;

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

    pub fn full_message(&self) -> &str {
        self.full_message
    }

    pub fn sha(&self) -> &str {
        self.sha
    }

    pub fn reverted_commit_sha(&self) -> Option<&str> {
        lazy_static! {
            static ref REVERTS_COMMIT_REGEX: Regex =
                Regex::new(r"This reverts commit ([a-zA-Z0-9]+).").unwrap();
        }

        REVERTS_COMMIT_REGEX
            .captures_iter(self.full_message)
            .filter_map(|capture| capture.get(1))
            .map(|group| group.as_str())
            .next()
    }

    pub fn markdown_text(&self, number: usize, status: CommitStatus) -> String {
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

        let (prefix, suffix) = match status {
            Both {
                reverts,
                is_reverted_by,
            } => (
                "<del>",
                format!("</del> (reverts [{reverts}], reverted by [{is_reverted_by}])"),
            ),
            IsRevertedBy(other_number) => {
                ("<del>", format!("</del> (reverted by [{other_number}])"))
            }
            Reverts(other_number) => ("<ins>", format!("</ins> (reverts [{other_number}])")),
            Normal => ("", String::new()),
        };

        let main_content = format!("- {prefix}{message} [[{number}]]({commit_url}){suffix}\n");
        let details = match message_lines.len() {
            2.. => format!("\n    {}", message_lines[1..].join("\n    ")),
            _ => String::new(),
        };

        main_content + &details
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;
    use test_case::test_case;

    use super::*;
    use crate::platform::Platform::{self, *};

    #[test_case(
        Android, "Test commit.", "abcdef", Normal,
        "- Test commit. [[2]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n";
        "Android: one line"
    )]
    #[test_case(
        Android, "Test commit.\nAnother line.", "abcdef", Normal,
        "- Test commit. [[2]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.";
        "Android: two lines"
    )]
    #[test_case(
        Android, "Test commit.\nAnother line.\nAnd another line.", "abcdef", Normal,
        "- Test commit. [[2]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.\n    And another line.";
        "Android: three lines"
    )]
    #[test_case(
        Android, "Test commit.\nCo-Authored-By: user", "abcdef", Normal,
        "- Test commit. [[2]](https://github.com/signalapp/Signal-Android/commit/abcdef)\n";
        "Android: Co-Authored-By is removed"
    )]
    #[test_case(
        Android, "Revert \"Test commit\".\nThis reverts commit fedcba.", "abcdef", Reverts(1),
        "- <ins>Revert \"Test commit\". [[2]](https://github.com/signalapp/Signal-Android/commit/abcdef)</ins> (reverts [1])\n";
        "Android: reverts commit"
    )]
    #[test_case(
        Android, "Test commit.", "abcdef", IsRevertedBy(3),
        "- <del>Test commit. [[2]](https://github.com/signalapp/Signal-Android/commit/abcdef)</del> (reverted by [3])\n";
        "Android: reverted commit"
    )]
    #[test_case(
        Android, "Test commit.", "abcdef", Both { reverts: 1, is_reverted_by: 3 },
        "- <del>Test commit. [[2]](https://github.com/signalapp/Signal-Android/commit/abcdef)</del> (reverts [1], reverted by [3])\n";
        "Android: reverted commit that reverts"
    )]
    #[test_case(
        Desktop, "Test commit.", "abcdef", Normal,
        "- Test commit. [[2]](https://github.com/signalapp/Signal-Desktop/commit/abcdef)\n";
        "Desktop: one line"
    )]
    #[test_case(
        Desktop, "Test commit. Test @mention!\nTest@mention2.", "abcdef", Normal,
        "- Test commit. Test `@mention`! [[2]](https://github.com/signalapp/Signal-Desktop/commit/abcdef)\n\n    Test`@mention2`.";
        "Desktop: two lines with mention"
    )]
    fn commit_markdown(
        platform: Platform,
        full_message: &str,
        sha: &str,
        status: CommitStatus,
        result: &str,
    ) {
        assert_str_eq!(
            Commit::new(platform, full_message, sha).markdown_text(2, status),
            result
        );
    }
}
