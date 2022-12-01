use lazy_static::lazy_static;
use regex::Regex;

use super::CommitStatus::{self, *};
use crate::{github, platform::Platform, utils};

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

    pub fn is_likely_localization_change(&self) -> bool {
        let lowercase = self.full_message.to_lowercase();

        lowercase.contains("language")
            || lowercase.contains("translation")
            || lowercase.contains("string")
            || lowercase.contains("release note")
            || lowercase.contains("i18n")
            || lowercase.contains("l10n")
            || lowercase.contains("update messages")
            || lowercase.contains("updated messages")
            || lowercase.contains("updates messages")
    }

    pub fn sha(&self) -> &str {
        self.sha
    }

    pub fn full_message(&self) -> &str {
        self.full_message
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

        let message_lines: Vec<_> = self
            .full_message
            .split('\n')
            .filter(|line| {
                let lowercase = line.to_lowercase();
                !lowercase.contains("co-authored-by") && !lowercase.contains("this reverts commit")
            })
            .map(|line| MENTION_REGEX.replace_all(line, "`@$1`"))
            .map(|line| utils::escape_html(&line))
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

        let description_exists = message_lines.len() >= 2;
        let description_omitted = description_exists && !self.platform.should_show_commit_details();
        let description_omitted_notice = if description_omitted { "[…] " } else { "" };

        let main_content = format!(
            "- {prefix}{message} {description_omitted_notice}[[{number}]]({commit_url}){suffix}\n"
        );
        let details = if description_exists && self.platform.should_show_commit_details() {
            format!("\n    {}", message_lines[1..].join("\n    "))
        } else {
            String::new()
        };

        main_content + &details
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::{assert_eq, assert_str_eq};
    use strum::IntoEnumIterator;
    use test_case::test_case;

    use super::*;
    use crate::platform::Platform::{self, *};

    #[test_case(true, "Updated language translations.")]
    #[test_case(true, "Update strings")]
    #[test_case(true, "Updates strings")]
    #[test_case(true, "Updates i18n strings")]
    #[test_case(true, "Update translations")]
    #[test_case(true, "Update release notes")]
    #[test_case(true, "Update release notes & App Store descriptions")]
    #[test_case(true, "Update messages")]
    #[test_case(true, "Updates messages")]
    #[test_case(true, "Updated messages")]
    #[test_case(false, "Update GitHub Actions")]
    #[test_case(false, "Test commit.")]
    fn is_likely_localization_change(result: bool, message: &str) {
        for platform in Platform::iter() {
            assert_eq!(
                Commit::new(platform, message, "abcdef").is_likely_localization_change(),
                result
            );
        }
    }

    #[test_case(
        Android, "Test commit.", "abcdef", Normal,
        "- Test commit. [[2]](//github.com/signalapp/Signal-Android/commit/abcdef)\n";
        "Android: one line"
    )]
    #[test_case(
        Android, "Test commit.\nAnother line.", "abcdef", Normal,
        "- Test commit. [[2]](//github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.";
        "Android: two lines"
    )]
    #[test_case(
        Android, "Test commit.\nAnother line.\nAnd another line.", "abcdef", Normal,
        "- Test commit. [[2]](//github.com/signalapp/Signal-Android/commit/abcdef)\n\n    Another line.\n    And another line.";
        "Android: three lines"
    )]
    #[test_case(
        Android, "Test commit.\nCo-Authored-By: user", "abcdef", Normal,
        "- Test commit. [[2]](//github.com/signalapp/Signal-Android/commit/abcdef)\n";
        "Android: Co-Authored-By is removed"
    )]
    #[test_case(
        Android, "Test commit.\nCo-authored-by: user", "abcdef", Normal,
        "- Test commit. [[2]](//github.com/signalapp/Signal-Android/commit/abcdef)\n";
        "Android: Co-Authored-By in any case is removed"
    )]
    #[test_case(
        Android, "Revert \"Test commit\".\nThis reverts commit fedcba.", "abcdef", Reverts(1),
        "- <ins>Revert &quot;Test commit&quot;. [[2]](//github.com/signalapp/Signal-Android/commit/abcdef)</ins> (reverts [1])\n";
        "Android: reverts commit"
    )]
    #[test_case(
        Android, "Test commit.", "abcdef", IsRevertedBy(3),
        "- <del>Test commit. [[2]](//github.com/signalapp/Signal-Android/commit/abcdef)</del> (reverted by [3])\n";
        "Android: reverted commit"
    )]
    #[test_case(
        Android, "Test commit.", "abcdef", Both { reverts: 1, is_reverted_by: 3 },
        "- <del>Test commit. [[2]](//github.com/signalapp/Signal-Android/commit/abcdef)</del> (reverts [1], reverted by [3])\n";
        "Android: reverted commit that reverts"
    )]
    #[test_case(
        Desktop, "Test commit.", "abcdef", Normal,
        "- Test commit. [[2]](//github.com/signalapp/Signal-Desktop/commit/abcdef)\n";
        "Desktop: one line"
    )]
    #[test_case(
        Desktop, "Test commit. Test @mention!\nTest@mention2.", "abcdef", Normal,
        "- Test commit. Test `@mention`! [[2]](//github.com/signalapp/Signal-Desktop/commit/abcdef)\n\n    Test`@mention2`.";
        "Desktop: two lines with mention"
    )]
    #[test_case(
        Desktop, "Test commit. Test <HtmlTag/>!\n<AnotherTag>Test!</AnotherTag>.", "abcdef", Normal,
        "- Test commit. Test &lt;HtmlTag/&gt;! [[2]](//github.com/signalapp/Signal-Desktop/commit/abcdef)\n\n    &lt;AnotherTag&gt;Test!&lt;/AnotherTag&gt;.";
        "Desktop: two lines with HTML"
    )]
    #[test_case(
        Ios, "Test commit. Continuation.", "abcdef", Normal,
        "- Test commit. Continuation. [[2]](//github.com/signalapp/Signal-iOS/commit/abcdef)\n";
        "iOS: one line"
    )]
    #[test_case(
        Ios, "Test commit. Continuation.\nContinuation 2.\nContinuation 3.", "abcdef", Normal,
        "- Test commit. Continuation. […] [[2]](//github.com/signalapp/Signal-iOS/commit/abcdef)\n";
        "iOS: three lines, details are not shown"
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
