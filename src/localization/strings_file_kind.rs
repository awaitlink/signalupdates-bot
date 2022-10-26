use std::fmt;

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::platform::Platform::{self, *};

#[derive(
    Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, EnumIter,
)]
pub enum StringsFileKind {
    Main,
    InfoPlist,
    PluralAware,
    AppStoreDescription,
    AppStoreReleaseNotes,
}

use StringsFileKind::*;

impl StringsFileKind {
    pub const fn applicable_for_platform(&self, platform: Platform) -> bool {
        matches!((platform, self), (Android | Desktop, Main) | (Ios, _))
    }

    pub fn applicable_iter(platform: Platform) -> impl Iterator<Item = Self> {
        Self::iter().filter(move |kind| kind.applicable_for_platform(platform))
    }

    pub const fn path_base(&self, platform: Platform) -> &'static str {
        match (platform, self) {
            (Android, _) => "app/src/main/res",
            (Ios, Main | InfoPlist | PluralAware) => "Signal/translations",
            (Ios, AppStoreDescription | AppStoreReleaseNotes) => "fastlane/metadata",
            (Desktop, _) => "_locales",
        }
    }

    /// Expects `language_placeholder_value` to be in the expected format for `platform` and `kind`.
    fn path_folder_name(&self, platform: Platform, language_placeholder_value: &str) -> String {
        if !self.applicable_for_platform(platform) {
            panic!("unexpected strings file for {platform}: {self:?}")
        }

        match (platform, self) {
            (Android, _) => format!("values-{language_placeholder_value}"),
            (Ios, Main | InfoPlist | PluralAware) => {
                format!("{language_placeholder_value}.lproj")
            }
            (Ios, AppStoreDescription | AppStoreReleaseNotes) | (Desktop, _) => {
                language_placeholder_value.to_owned()
            }
        }
    }

    pub const fn path_file_name(&self, platform: Platform) -> &'static str {
        match (platform, self) {
            (Android, _) => "strings.xml",
            (Ios, Main) => "Localizable.strings",
            (Ios, InfoPlist) => "InfoPlist.strings",
            (Ios, PluralAware) => "PluralAware.stringsdict",
            (Ios, AppStoreDescription) => "description.txt",
            (Ios, AppStoreReleaseNotes) => "release_notes.txt",
            (Desktop, _) => "messages.json",
        }
    }

    /// Expects `language_placeholder_value` to be in the expected format for `platform` and `kind`.
    pub fn path(&self, platform: Platform, language_placeholder_value: &str) -> String {
        format!(
            "{}/{}/{}",
            self.path_base(platform),
            self.path_folder_name(platform, language_placeholder_value),
            self.path_file_name(platform),
        )
    }
}

impl fmt::Display for StringsFileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Main => "main",
                InfoPlist => "info",
                PluralAware => "plural",
                AppStoreDescription => "desc",
                AppStoreReleaseNotes => "release",
            }
        )
    }
}
