use std::fmt;

use semver::Version;
use strum_macros::EnumIter;

use crate::utils;

pub const ANDROID_DEFAULT_STRINGS_FILENAME: &str = "app/src/main/res/values/strings.xml";

#[derive(Debug, Clone, Copy, EnumIter, PartialEq, Eq)]
pub enum Platform {
    Android,
    Ios,
    Desktop,
}

use Platform::*;

impl Platform {
    pub fn github_api_tags_url(&self) -> String {
        format!("https://api.github.com/repos/signalapp/Signal-{self}/tags")
    }

    pub fn should_post_version(&self, version: &Version) -> bool {
        match self {
            Android => version.build.is_empty(), // versions like 1.2.3.4 are filtered out (the "4" is parsed into `build` by lenient_semver)
            Ios | Desktop => version.pre.contains("beta"),
        }
    }

    pub fn github_api_comparison_url(&self, old: &str, new: &str) -> String {
        format!("https://api.github.com/repos/signalapp/Signal-{self}/compare/{old}...{new}")
    }

    pub fn github_comparison_url(&self, old: &str, new: &str, file_path: Option<&str>) -> String {
        match file_path {
            Some(file_path) => format!(
                "https://github.com/signalapp/Signal-{self}/compare/{old}..{new}#diff-{}", // note: using `..` instead of `...`
                utils::sha256_string(file_path)
            ),
            None => format!("https://github.com/signalapp/Signal-{self}/compare/{old}...{new}"),
        }
    }

    pub fn github_api_commit_url(&self, sha: &str) -> String {
        format!("https://api.github.com/repos/signalapp/Signal-{self}/commits/{sha}")
    }

    pub fn github_commit_url(&self, sha: &str) -> String {
        format!("https://github.com/signalapp/Signal-{self}/commit/{sha}")
    }

    pub const fn availability_notice(&self) -> &'static str {
        match self {
            Android => "\n(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)",
            Ios | Desktop => "",
        }
    }

    pub fn discourse_topic_slug_url(&self, version: &Version) -> String {
        format!(
            "https://community.signalusers.org/t/beta-feedback-for-the-upcoming-{}-{}-{}-release.json",
            self.to_string().to_ascii_lowercase(), version.major, version.minor
        )
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Android => "Android",
                Ios => "iOS",
                Desktop => "Desktop",
            }
        )
    }
}
