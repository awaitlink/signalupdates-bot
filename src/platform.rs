use lazy_static::lazy_static;
use regex::Regex;
use semver::Version;
use std::fmt;
use Platform::*;

use crate::{
    language::{Language, LocalizationChange},
    utils,
};

pub const ANDROID_DEFAULT_STRINGS_FILENAME: &str = "app/src/main/res/values/strings.xml";

#[derive(Debug, Clone, Copy, strum_macros::EnumIter)]
pub enum Platform {
    Android,
    Desktop,
}

impl Platform {
    pub const fn github_api_tags_url(&self) -> &'static str {
        match self {
            Android => "https://api.github.com/repos/signalapp/Signal-Android/tags",
            Desktop => "https://api.github.com/repos/signalapp/Signal-Desktop/tags",
        }
    }

    pub fn should_post_version(&self, version: &Version) -> bool {
        match self {
            Android => version.build.is_empty(), // versions like 1.2.3.4 are filtered out (the "4" is parsed into `build` by lenient_semver)
            Desktop => version.pre.contains("beta"),
        }
    }

    pub fn github_api_comparison_url(
        &self,
        old: &str,
        new: &str,
        page: usize,
        per_page: usize,
    ) -> String {
        format!("https://api.github.com/repos/signalapp/Signal-{self}/compare/{old}...{new}?page={page}&per_page={per_page}")
    }

    pub fn github_comparison_url(&self, old: &str, new: &str, filename: Option<&str>) -> String {
        match filename {
            Some(filename) => format!(
                "https://github.com/signalapp/Signal-{self}/compare/{old}...{new}#diff-{}",
                utils::sha256_string(filename)
            ),
            None => format!("https://github.com/signalapp/Signal-{self}/compare/{old}...{new}"),
        }
    }

    pub fn github_commit_url(&self, sha: &str) -> String {
        format!("https://github.com/signalapp/Signal-{self}/commit/{sha}")
    }

    pub const fn availability_notice(&self) -> &'static str {
        match self {
            Android => "\n(Not Yet) Available via [Firebase App Distribution](https://community.signalusers.org/t/17538)",
            Desktop => "",
        }
    }

    pub fn discourse_topic_slug_url(&self, version: &Version) -> String {
        format!(
            "https://community.signalusers.org/t/beta-feedback-for-the-upcoming-{}-{}-{}-release.json",
            self.to_string().to_ascii_lowercase(), version.major, version.minor
        )
    }

    pub fn localization_change(&self, filename: &str) -> Option<LocalizationChange> {
        lazy_static! {
            static ref ANDROID_REGEX: Regex =
                Regex::new(r"app/src/main/res/values-([a-zA-Z]{2,3}(-r[A-Z]{2})?)/strings\.xml")
                    .unwrap();
            static ref DESKTOP_REGEX: Regex =
                Regex::new(r"_locales/([a-zA-Z]{2,3}(_[A-Z]{2})?)/messages\.json").unwrap();
        }

        let captures_iter = match self {
            Android => {
                if filename == ANDROID_DEFAULT_STRINGS_FILENAME {
                    return Some(LocalizationChange {
                        language: Language::default(),
                        filename: ANDROID_DEFAULT_STRINGS_FILENAME.to_string(),
                    });
                }

                ANDROID_REGEX.captures_iter(filename)
            }
            Desktop => DESKTOP_REGEX.captures_iter(filename),
        };

        captures_iter
            .filter_map(|captures| captures.get(1))
            .map(|capture| capture.as_str())
            .find_map(Language::from_code)
            .map(|language| LocalizationChange {
                language,
                filename: filename.to_string(),
            })
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Android => "Android",
                Desktop => "Desktop",
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(Android, "app/src/main/res/values/strings.xml", "English (en)"; "Android: en")]
    #[test_case(Android, "app/src/main/res/values-kab/strings.xml", "Kabyle (kab)"; "Android: kab")]
    #[test_case(Android, "app/src/main/res/values-pa-rPK/strings.xml", "Panjabi (Pakistan) (pa-PK)"; "Android: pa dash r PK")]
    #[test_case(Desktop, "_locales/en/messages.json", "English (en)"; "Desktop: en")]
    #[test_case(Desktop, "_locales/kab/messages.json", "Kabyle (kab)"; "Desktop: kab")]
    #[test_case(Desktop, "_locales/pa_PK/messages.json", "Panjabi (Pakistan) (pa-PK)"; "Desktop: pa underscore PK")]
    fn localization_change_language(platform: Platform, filename: &str, result: &str) {
        assert_eq!(
            platform
                .localization_change(filename)
                .unwrap()
                .language
                .to_string(),
            result
        );
    }

    // Some of the values-* folders in Signal Android are not for localization.
    #[test_case(Android, "app/src/main/res/values-land/strings.xml"; "land")]
    #[test_case(Android, "app/src/main/res/values-v9/strings.xml"; "v9")]
    fn localization_change_none(platform: Platform, filename: &str) {
        assert!(platform.localization_change(filename).is_none());
    }
}
