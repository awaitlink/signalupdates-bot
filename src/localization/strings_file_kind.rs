use std::fmt;

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::platform::Platform::{self, *};

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumIter)]
pub enum StringsFileKind {
    Main,
    InfoPlist,
    PluralAware,
    AppStoreDescription,
    AppStoreReleaseNotes,
}

use StringsFileKind::*;

impl StringsFileKind {
    pub fn applicable_for_platform(&self, platform: Platform) -> bool {
        matches!((platform, self), (Android | Desktop, Main) | (Ios, _))
    }

    pub fn applicable_iter(platform: Platform) -> impl Iterator<Item = Self> {
        Self::iter().filter(move |kind| kind.applicable_for_platform(platform))
    }

    /// Expects `language_placeholder_value` to be in the expected format for `platform` and `kind`.
    pub fn path(&self, platform: Platform, language_placeholder_value: &str) -> String {
        if !self.applicable_for_platform(platform) {
            panic!("unexpected strings file for {platform}: {:?}", self)
        }

        match platform {
            Android => format!(
                "app/src/main/res/values-{}/strings.xml",
                language_placeholder_value
            ),
            Ios => match self {
                Main | InfoPlist | PluralAware => format!(
                    "Signal/translations/{}.lproj/{}",
                    language_placeholder_value,
                    match self {
                        Main => "Localizable.strings",
                        InfoPlist => "InfoPlist.strings",
                        PluralAware => "PluralAware.stringsdict",
                        _ => unreachable!(),
                    }
                ),
                AppStoreDescription | AppStoreReleaseNotes => format!(
                    "fastlane/metadata/{}/{}",
                    language_placeholder_value,
                    match self {
                        AppStoreDescription => "description.txt",
                        AppStoreReleaseNotes => "release_notes.txt",
                        _ => unreachable!(),
                    }
                ),
            },

            Desktop => format!("_locales/{}/messages.json", language_placeholder_value,),
        }
    }
}

impl fmt::Display for StringsFileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Main => "main",
                InfoPlist => "info plist",
                PluralAware => "plural aware",
                AppStoreDescription => "App Store description",
                AppStoreReleaseNotes => "App Store release notes",
            }
        )
    }
}
