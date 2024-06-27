use std::fmt;

use semver::Version;
use strum_macros::EnumIter;

use crate::utils;

pub mod android;

pub const ANDROID_DEFAULT_STRINGS_FILENAME: &str = "app/src/main/res/values/strings.xml";
pub const SERVER_STRINGS_FILENAME: &str =
    "service/src/main/resources/org/signal/badges/Badges.properties";

#[derive(Debug, Clone, Copy, EnumIter, PartialEq, Eq, Hash)]
pub enum Platform {
    Android,
    Ios,
    Desktop,
    Server,
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
            Server => true,
        }
    }

    pub fn should_show_commit(&self, full_message: &str) -> bool {
        match self {
            Android | Desktop | Server => true,
            Ios => {
                !full_message.contains("Bump build to")
                    && !full_message.contains("Feature flags for")
            }
        }
    }

    pub fn should_show_commit_details(&self) -> bool {
        matches!(self, Android | Desktop | Server)
    }

    pub fn github_api_comparison_url(&self, old: &str, new: &str) -> String {
        format!("https://api.github.com/repos/signalapp/Signal-{self}/compare/{old}...{new}")
    }

    pub fn github_comparison_url(
        &self,
        old: &str,
        new: &str,
        file_path: Option<&str>,
        specify_protocol: bool,
    ) -> String {
        let base = match file_path {
            Some(file_path) => format!(
                "//github.com/signalapp/Signal-{self}/compare/{old}..{new}#diff-{}", // note: using `..` instead of `...`
                utils::sha256_string(file_path)
            ),
            None => format!("//github.com/signalapp/Signal-{self}/compare/{old}...{new}"),
        };

        format!("{}{base}", if specify_protocol { "https:" } else { "" })
    }

    pub fn github_api_commit_url(&self, sha: &str) -> String {
        format!("https://api.github.com/repos/signalapp/Signal-{self}/commits/{sha}")
    }

    pub fn github_commit_url(&self, sha: &str) -> String {
        format!("//github.com/signalapp/Signal-{self}/commit/{sha}")
    }

    pub fn github_raw_url(&self, revision: &str) -> String {
        format!("https://raw.githubusercontent.com/signalapp/Signal-{self}/{revision}")
    }

    pub const fn availability_notice(&self, available: bool) -> &'static str {
        match self {
            Android => {
                if available {
                    "\nAvailable via [Firebase App Distribution](/t/17538) despite [this](/t/17538/114)? :eyes:"
                } else {
                    "\nBuilds [will no longer be published to Firebase App Distribution](/t/17538/114)"
                }
            }
            Ios | Desktop | Server => "",
        }
    }

    pub fn discourse_topic_slug_url(
        &self,
        version: &Version,
        topic_id_for_server_updates: u64,
    ) -> String {
        match self {
            Android | Ios | Desktop => format!(
                "https://community.signalusers.org/t/beta-feedback-for-the-upcoming-{}-{}-{}-release.json",
                self.to_string().to_ascii_lowercase(), version.major, version.minor
            ),
            Server => format!(
                "https://community.signalusers.org/t/{}.json", topic_id_for_server_updates)
        }
    }

    pub fn state_key(&self) -> String {
        self.to_string().to_lowercase()
    }

    pub const fn color(&self) -> u64 {
        match self {
            Android => 0x1d8663,
            Ios => 0x336ba3,
            Desktop => 0xaa377a,
            Server => 0x6058ca,
        }
    }

    pub fn archiving_message_necessary(&self) -> bool {
        !matches!(self, Server)
    }

    pub fn repo_slug(&self) -> String {
        format!("signalapp/Signal-{self}")
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
                Server => "Server",
            }
        )
    }
}
