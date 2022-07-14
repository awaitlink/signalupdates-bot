use std::collections::{HashMap, HashSet};

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    localization::{
        Language,
        StringsFileKind::{self, *},
    },
    platform::{
        Platform::{self, *},
        ANDROID_DEFAULT_STRINGS_FILENAME,
    },
    types::github::{File, Tag},
};

pub type UnsortedChanges = HashMap<Language, HashSet<StringsFileKind>>;

// TODO: the '.' from StringsFileKind::path() will be interpreted as "any character", not just '.'

const ANDROID_LANGUAGE_CODE_PATTERN: &str = "([a-zA-Z]{2,3}(-r[A-Z]{2})?)";
const IOS_DESKTOP_LANGUAGE_CODE_PATTERN: &str = "([a-zA-Z]{2,3}(_[A-Z]{2})?)";
const IOS_APP_STORE_LANGUAGE_CODE_PATTERN: &str = "([a-zA-Z]{2,3}(-[a-zA-Z]{2,4})?)";

fn regex(platform: Platform, kind: StringsFileKind, pattern: &str) -> Regex {
    Regex::new(&kind.path(platform, pattern)).unwrap()
}

lazy_static! {
    // Android
    static ref ANDROID_REGEX: Regex =
        regex(Android, Main, ANDROID_LANGUAGE_CODE_PATTERN);

    // iOS
    static ref IOS_MAIN_REGEX: Regex =
        regex(Ios, Main, IOS_DESKTOP_LANGUAGE_CODE_PATTERN);
    static ref IOS_INFO_PLIST_REGEX: Regex =
        regex(Ios, InfoPlist, IOS_DESKTOP_LANGUAGE_CODE_PATTERN);
    static ref IOS_PLURAL_AWARE_REGEX: Regex =
        regex(Ios, PluralAware, IOS_DESKTOP_LANGUAGE_CODE_PATTERN);
    static ref IOS_APP_STORE_DESCRIPTION_REGEX: Regex =
        regex(Ios, AppStoreDescription, IOS_APP_STORE_LANGUAGE_CODE_PATTERN);
    static ref IOS_APP_STORE_RELEASE_NOTES_REGEX: Regex =
        regex(Ios, AppStoreReleaseNotes, IOS_APP_STORE_LANGUAGE_CODE_PATTERN);

    // Desktop
    static ref DESKTOP_REGEX: Regex =
        regex(Desktop, Main, IOS_DESKTOP_LANGUAGE_CODE_PATTERN);
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalizationChange {
    language: Language,
    kinds: Vec<StringsFileKind>,
}

impl LocalizationChange {
    pub fn unsorted_changes_from_file_paths(
        platform: Platform,
        file_paths: &[&str],
    ) -> UnsortedChanges {
        let pairs = file_paths.iter().filter_map(|filename| {
            StringsFileKind::applicable_iter(platform).find_map(move |kind| {
                let regex = match platform {
                    Android => {
                        if filename == &ANDROID_DEFAULT_STRINGS_FILENAME {
                            return Some((Language::default(), Main));
                        }

                        &*ANDROID_REGEX
                    }
                    Ios => match kind {
                        Main => &*IOS_MAIN_REGEX,
                        InfoPlist => &*IOS_INFO_PLIST_REGEX,
                        PluralAware => &*IOS_PLURAL_AWARE_REGEX,
                        AppStoreDescription => &*IOS_APP_STORE_DESCRIPTION_REGEX,
                        AppStoreReleaseNotes => &*IOS_APP_STORE_RELEASE_NOTES_REGEX,
                    },
                    Desktop => &*DESKTOP_REGEX,
                };

                regex
                    .captures_iter(filename)
                    .filter_map(|captures| captures.get(1))
                    .map(|capture| capture.as_str())
                    .find_map(|language_code| Language::try_from(language_code).ok())
                    .map(|language| (language, kind))
            })
        });

        let mut map: UnsortedChanges = HashMap::new();

        for (language, kind) in pairs {
            map.entry(language)
                .or_insert_with(HashSet::new)
                .insert(kind);
        }

        map
    }

    pub fn unsorted_changes_from_files(
        platform: Platform,
        files: &Option<Vec<File>>,
    ) -> UnsortedChanges {
        Self::unsorted_changes_from_file_paths(
            platform,
            &files
                .as_ref()
                .unwrap()
                .iter()
                .map(|file| file.filename.as_str())
                .collect::<Vec<_>>(),
        )
    }

    pub fn merge_unsorted_changes(items: Vec<&mut UnsortedChanges>) -> UnsortedChanges {
        let mut map: UnsortedChanges = HashMap::new();

        for unsorted_changes in items {
            for (language, kinds) in unsorted_changes {
                map.entry(language.clone())
                    .or_insert_with(HashSet::new)
                    .extend(kinds.iter().copied());
            }
        }

        map
    }

    pub fn sorted_changes(unsorted_changes: UnsortedChanges) -> Vec<Self> {
        let mut changes: Vec<_> = unsorted_changes
            .into_iter()
            .map(|(language, kinds)| {
                let mut kinds: Vec<_> = kinds.into_iter().collect();
                kinds.sort_unstable();
                LocalizationChange { language, kinds }
            })
            .collect();

        changes.sort_unstable();

        changes
    }

    pub fn string(&self, platform: Platform, old_tag: &Tag, new_tag: &Tag) -> String {
        match (platform, &self.kinds[..]) {
            (Android | Desktop, &[Main]) => format!(
                "[{}]({})",
                self.language,
                platform.github_comparison_url(
                    &old_tag.name,
                    &new_tag.name,
                    Some(&self.file_paths(platform)[0])
                )
            ),
            _ => format!(
                "{}: {}",
                self.language,
                self.kinds
                    .iter()
                    .zip(self.file_paths(platform).into_iter())
                    .map(|(kind, path)| {
                        format!(
                            "[{}]({})",
                            kind,
                            platform.github_comparison_url(
                                &old_tag.name,
                                &new_tag.name,
                                Some(&path)
                            )
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" • ")
            ),
        }
    }

    pub fn file_paths(&self, platform: Platform) -> Vec<String> {
        self.kinds
            .iter()
            .map(|kind| {
                if !kind.applicable_for_platform(platform) {
                    panic!("unexpected strings file for {platform}: {kind:?}")
                }

                match (platform, kind) {
                    (Android, _) => match self.language.full_code().as_str() {
                        "en" => ANDROID_DEFAULT_STRINGS_FILENAME.to_owned(),
                        _ => kind.path(platform, &self.language.full_code().replace('-', "-r")),
                    },
                    (Ios, Main | InfoPlist | PluralAware) | (Desktop, _) => {
                        kind.path(platform, &self.language.full_code().replace('-', "_"))
                    }
                    (Ios, AppStoreDescription | AppStoreReleaseNotes) => {
                        kind.path(platform, &self.language.full_code())
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
impl LocalizationChange {
    pub fn test_change(language_code: &str, kinds: Vec<StringsFileKind>) -> LocalizationChange {
        LocalizationChange {
            language: Language::try_from(language_code).unwrap(),
            kinds,
        }
    }

    pub fn language(&self) -> &Language {
        &self.language
    }

    pub fn kinds(&self) -> &[StringsFileKind] {
        &self.kinds
    }

    pub fn unsorted_changes(sorted_changes: Vec<LocalizationChange>) -> UnsortedChanges {
        sorted_changes
            .into_iter()
            .map(|change| {
                (
                    change.language().clone(),
                    change.kinds().iter().copied().collect::<HashSet<_>>(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::{assert_eq, assert_str_eq};
    use test_case::test_case;

    use super::*;

    #[ignore = "online and doesn't actually test"]
    #[test]
    fn online_localization_change_all() {
        use crate::{
            platform::Platform::{self, *},
            types::github,
        };

        #[allow(clippy::type_complexity)]
        let platforms: [(Platform, &[(&str, &[&str])]); 3] = [
            (Android, &[("app/src/main/res", &["strings.xml"])]),
            (
                Ios,
                &[
                    (
                        "Signal/translations",
                        &[
                            "Localizable.strings",
                            "InfoPlist.strings",
                            "PluralAware.stringsdict",
                        ],
                    ),
                    (
                        "fastlane/metadata",
                        &["description.txt", "release_notes.txt"],
                    ),
                ],
            ),
            (Desktop, &[("_locales", &["messages.json"])]),
        ];

        let client = reqwest::blocking::Client::builder()
            .user_agent(crate::utils::USER_AGENT)
            .build()
            .expect("client should be built successfully");

        for (platform, pairs) in platforms {
            for (path, filenames) in pairs {
                let url = format!(
                    "https://api.github.com/repos/signalapp/Signal-{platform}/contents/{path}"
                );

                let entries: Vec<github::ContentsEntry> = client
                    .get(url)
                    .send()
                    .expect("request should succeed")
                    .json()
                    .expect("json should be successfully deserialized");

                let file_paths: Vec<_> = entries
                    .into_iter()
                    .map(|entry| entry.path)
                    .flat_map(|path| {
                        filenames
                            .iter()
                            .map(move |filename| format!("{path}/{filename}"))
                    })
                    .collect();

                let file_paths: Vec<_> = file_paths
                    .iter()
                    .map(|full_path| full_path.as_str())
                    .collect();

                let changes = LocalizationChange::unsorted_changes_from_file_paths(
                    platform,
                    file_paths.as_slice(),
                );

                let sorted = LocalizationChange::sorted_changes(changes);

                println!(
                    "{platform} ({path}): {:#?}",
                    sorted
                        .iter()
                        .map(|change| change.language.to_string())
                        .collect::<Vec<_>>()
                );
            }
        }

        panic!("testing");
    }

    #[test_case(Android, "app/src/main/res/values/strings.xml", "English (`en`)"; "Android: en")]
    #[test_case(Android, "app/src/main/res/values-kab/strings.xml", "Kabyle (`kab`)"; "Android: kab")]
    #[test_case(Android, "app/src/main/res/values-pa-rPK/strings.xml", "Panjabi (`pa-PK`)"; "Android: pa dash r PK")]
    #[test_case(Desktop, "_locales/en/messages.json", "English (`en`)"; "Desktop: en")]
    #[test_case(Desktop, "_locales/kab/messages.json", "Kabyle (`kab`)"; "Desktop: kab")]
    #[test_case(Desktop, "_locales/pa_PK/messages.json", "Panjabi (`pa-PK`)"; "Desktop: pa underscore PK")]
    #[test_case(Ios, "Signal/translations/en.lproj/Localizable.strings", "English (`en`)"; "iOS main: en")]
    #[test_case(Ios, "Signal/translations/kab.lproj/Localizable.strings", "Kabyle (`kab`)"; "iOS main: kab")]
    #[test_case(Ios, "Signal/translations/pa_PK.lproj/Localizable.strings", "Panjabi (`pa-PK`)"; "iOS main: pa underscore PK")]
    #[test_case(Ios, "Signal/translations/en.lproj/InfoPlist.strings", "English (`en`)"; "iOS info plist: en")]
    #[test_case(Ios, "Signal/translations/en.lproj/PluralAware.stringsdict", "English (`en`)"; "iOS plural aware: en")]
    #[test_case(Ios, "fastlane/metadata/en-US/description.txt", "English (`en-US`)"; "iOS app store description: en dash US")]
    #[test_case(Ios, "fastlane/metadata/en-US/release_notes.txt", "English (`en-US`)"; "iOS app store release notes: en dash US")]
    fn localization_change_language(platform: Platform, file_path: &str, result: &str) {
        assert_eq!(
            LocalizationChange::sorted_changes(
                LocalizationChange::unsorted_changes_from_file_paths(platform, &[file_path])
            )
            .iter()
            .map(LocalizationChange::language)
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
            vec![result]
        );
    }

    #[test_case(Ios, &[("en", 5)], &[
        "Signal/translations/en.lproj/Localizable.strings",
        "Signal/translations/en.lproj/InfoPlist.strings",
        "fastlane/metadata/en/description.txt",
        "Signal/translations/en.lproj/PluralAware.stringsdict",
        "fastlane/metadata/en/release_notes.txt",
    ]; "iOS all en")]
    #[test_case(Ios, &[("en", 3), ("en-US", 2)], &[
        "Signal/translations/en.lproj/InfoPlist.strings",
        "fastlane/metadata/en-US/description.txt",
        "Signal/translations/en.lproj/Localizable.strings",
        "fastlane/metadata/en-US/release_notes.txt",
        "Signal/translations/en.lproj/PluralAware.stringsdict",
    ]; "iOS en and en dash US")]
    fn localization_change_multiple_languages_multiple_kinds(
        platform: Platform,
        languages: &[(&str, usize)],
        file_paths: &[&str],
    ) {
        let result = LocalizationChange::sorted_changes(
            LocalizationChange::unsorted_changes_from_file_paths(platform, file_paths),
        );

        dbg!(&result);
        assert_eq!(result.len(), languages.len());

        let mut total_kinds_count = 0;

        for (language, kinds_count) in languages {
            total_kinds_count += kinds_count;

            assert_eq!(
                &result
                    .iter()
                    .find(|change| &change.language().full_code() == language)
                    .unwrap()
                    .kinds()
                    .len(),
                kinds_count
            );
        }

        assert_eq!(total_kinds_count, file_paths.len(), "invalid test");
    }

    #[test_case(Android, "app/src/main/res/values-land/strings.xml"; "Android: land")]
    #[test_case(Android, "app/src/main/res/values-v9/strings.xml"; "Android: v9")]
    #[test_case(Ios, "Signal/translations/.tx/signal-ios.localizablestrings-30/de_translation"; "iOS: dot tx 1")]
    #[test_case(Ios, "Signal/translations/.tx/signal-ios.localizablestrings-30/zh_CN_translation"; "iOS: dot tx 2")]
    #[test_case(Ios, "Signal/translations/.tx/signal-ios.localizablestrings-30/zh_TW.Big5_translation"; "iOS: dot tx 3")]
    fn localization_change_none(platform: Platform, file_path: &str) {
        assert_eq!(
            LocalizationChange::sorted_changes(
                LocalizationChange::unsorted_changes_from_file_paths(platform, &[file_path])
            ),
            vec![]
        );
    }

    #[test_case(Android, Main, "en", "app/src/main/res/values/strings.xml"; "Android: en")]
    #[test_case(Android, Main, "en-US", "app/src/main/res/values-en-rUS/strings.xml"; "Android: en dash US")]
    #[test_case(Desktop, Main, "en", "_locales/en/messages.json"; "Desktop: en")]
    #[test_case(Desktop, Main, "en-US", "_locales/en_US/messages.json"; "Desktop: en dash US")]
    #[test_case(Ios, Main, "en", "Signal/translations/en.lproj/Localizable.strings"; "iOS main: en")]
    #[test_case(Ios, Main, "en-US", "Signal/translations/en_US.lproj/Localizable.strings"; "iOS main: en dash US")]
    #[test_case(Ios, InfoPlist, "en", "Signal/translations/en.lproj/InfoPlist.strings"; "iOS info plist: en")]
    #[test_case(Ios, PluralAware, "en", "Signal/translations/en.lproj/PluralAware.stringsdict"; "iOS plural aware: en")]
    #[test_case(Ios, AppStoreDescription, "en-US", "fastlane/metadata/en-US/description.txt"; "iOS app store description: en dash US")]
    #[test_case(Ios, AppStoreReleaseNotes, "en-US", "fastlane/metadata/en-US/release_notes.txt"; "iOS app store release notes: en dash US")]
    fn path_for_language_code_and_reverse(
        platform: Platform,
        kind: StringsFileKind,
        language_code: &str,
        result: &str,
    ) {
        let localization_change = LocalizationChange {
            language: Language::try_from(language_code).unwrap(),
            kinds: vec![kind],
        };

        let file_paths = localization_change.file_paths(platform);
        assert_eq!(file_paths, vec![result]);

        let changes = LocalizationChange::sorted_changes(
            LocalizationChange::unsorted_changes_from_file_paths(
                platform,
                &file_paths
                    .iter()
                    .map(|string| string.as_str())
                    .collect::<Vec<_>>(),
            ),
        );
        assert_eq!(changes, vec![localization_change]);
    }

    #[test_case(
        Android, &[Main], "en",
        "[English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)";
        "Android: en"
    )]
    #[test_case(
        Desktop, &[Main], "en",
        "[English (`en`)](https://github.com/signalapp/Signal-Desktop/compare/v1.2.3..v1.2.4#diff-4362c7f7032e9687a0a5910cadc127afbe8259b2b941de40dd4246c35b1446f0)";
        "Desktop: en"
    )]
    #[test_case(
        Ios, &[Main, InfoPlist, PluralAware], "en",
        "English (`en`): [main](https://github.com/signalapp/Signal-iOS/compare/v1.2.3..v1.2.4#diff-e51dc1f3b323f252674c72d0a8c33e70ea2a9c4f0c7784bdc39bdf2bf166233b) • [info plist](https://github.com/signalapp/Signal-iOS/compare/v1.2.3..v1.2.4#diff-fa966e7c12e08d6d541dc0cc19dac11cc749da30a4c855f48eaea6d38ba6e370) • [plural aware](https://github.com/signalapp/Signal-iOS/compare/v1.2.3..v1.2.4#diff-b1406c86358c13ed48eee0e5f535316b4754e72e30b1318e2c85ca1d75125262)";
        "iOS main, info plist, plural aware: en"
    )]
    #[test_case(
        Ios, &[AppStoreDescription, AppStoreReleaseNotes], "en-US",
        "English (`en-US`): [description](https://github.com/signalapp/Signal-iOS/compare/v1.2.3..v1.2.4#diff-e7a69d0898d3b2197f77bec55cad6b6d2ff8c973b873bfbe0fe568a1c710ef9c) • [release notes](https://github.com/signalapp/Signal-iOS/compare/v1.2.3..v1.2.4#diff-4256fffd9552dba2d12fe36150428ff03b2ede950c4040c5d840a6d6b1240df8)";
        "iOS app store: en dash US"
    )]
    fn string(platform: Platform, kinds: &[StringsFileKind], language_code: &str, result: &str) {
        let localization_change = LocalizationChange {
            language: Language::try_from(language_code).unwrap(),
            kinds: kinds.to_vec(),
        };

        assert_str_eq!(
            localization_change.string(platform, &Tag::new("v1.2.3"), &Tag::new("v1.2.4")),
            result
        )
    }
}
