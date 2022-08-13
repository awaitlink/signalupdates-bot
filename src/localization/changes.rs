use std::fmt;

use log::*;

use crate::{
    github::{Comparison, Tag},
    localization::{
        Completeness::{self, *},
        LocalizationChange, UnsortedChanges,
    },
    platform::Platform,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationChanges<'a> {
    pub platform: Platform,
    pub old_tag: &'a Tag,
    pub new_tag: &'a Tag,
    pub completeness: Completeness,
    pub unsorted_changes: UnsortedChanges,
}

impl<'a> LocalizationChanges<'a> {
    /// Note: assumes `comparison.files` is not `None`.
    pub fn from_comparison(
        platform: Platform,
        old_tag: &'a Tag,
        new_tag: &'a Tag,
        comparison: &'a Comparison,
    ) -> LocalizationChanges<'a> {
        let complete = comparison.are_files_likely_complete().unwrap();
        debug!("complete = {}", complete);

        let changes = LocalizationChange::unsorted_changes_from_files(platform, &comparison.files);

        debug!("changes.len() = {:?}", changes.len());

        Self {
            platform,
            old_tag,
            new_tag,
            completeness: if complete { Complete } else { Incomplete },
            unsorted_changes: changes,
        }
    }

    pub fn add_unsorted_changes(&mut self, unsorted_changes: &mut UnsortedChanges) {
        self.unsorted_changes = LocalizationChange::merge_unsorted_changes(vec![
            &mut self.unsorted_changes,
            unsorted_changes,
        ]);
    }

    pub fn full_comparison_notice(&self) -> String {
        format!(
            "You can view the full comparison to {} so far [here]({}).",
            self.old_tag.exact_version_string(),
            self.platform
                .github_comparison_url(&self.old_tag.name, &self.new_tag.name, None)
        )
    }

    fn language_links(&self) -> String {
        LocalizationChange::sorted_changes(self.unsorted_changes.clone())
            .iter()
            .map(|change| change.string(self.platform, self.old_tag, self.new_tag))
            .collect::<Vec<_>>()
            .join("\n- ")
    }
}

impl fmt::Display for LocalizationChanges<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (at_least, warning) = match self.completeness {
            Incomplete | LikelyComplete => (
                "At least ",
                format!(
                    "\n{} {}",
                    self.completeness.warning_text(),
                    self.full_comparison_notice()
                ),
            ),
            Complete => ("", String::new()),
        };

        let old_version = self.old_tag.exact_version_string();
        let changes_len = self.unsorted_changes.len();

        let (prefix, suffix) = match changes_len {
            0..=20 => ("", ""),
            _ => ("\n[details=\"Show changes\"]", "\n[/details]"),
        };

        let changes = match changes_len {
            1.. => format!("\n- {}", self.language_links()),
            _ => String::from("\n*No localization changes found*"),
        };

        let languages_word_suffix = if changes_len == 1 { "" } else { "s" };

        write!(
            f,
            "#### {at_least}{changes_len} language{languages_word_suffix} changed since {old_version}:{warning}{prefix}{changes}{suffix}"
        )
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;
    use test_case::test_case;

    use super::*;
    use crate::{
        localization::{LocalizationChange, LocalizationChanges, StringsFileKind::*},
        platform::Platform::{self, *},
    };

    #[test_case(Android, "v1.2.3", "v1.2.4", Complete, vec![
        LocalizationChange::test_change("en", vec![Main])
    ], "#### 1 language changed since 1.2.3:
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)"; "Android: 1 language changed, complete")]
    #[test_case(Android, "v1.2.3", "v1.2.4", Complete, vec![
        LocalizationChange::test_change("en", vec![Main]),
        LocalizationChange::test_change("en-US", vec![Main])
    ], "#### 2 languages changed since 1.2.3:
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en-US`)](https://github.com/signalapp/Signal-Android/compare/v1.2.3..v1.2.4#diff-33a220e7f3b2e559ebec12cbf273da0c135bfade5a547e41e2bb5a66d75a01d2)"; "Android: 2 languages changed, complete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", Incomplete, vec![
        LocalizationChange::test_change("en", vec![Main]),
        LocalizationChange::test_change("en-US", vec![Main]),
        LocalizationChange::test_change("en-CA", vec![Main]),
    ], "#### At least 3 languages changed since 1.1.5:
:warning: For technical reasons, not all languages may be listed below. You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en-CA`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-11b72e6873b6a42a2a1b92999e4225d7ff2921e323baa5e7b31fdc49471d9724)
- [English (`en-US`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-33a220e7f3b2e559ebec12cbf273da0c135bfade5a547e41e2bb5a66d75a01d2)"; "Android: 3 languages changed, incomplete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", LikelyComplete, vec![
        LocalizationChange::test_change("en", vec![Main]),
        LocalizationChange::test_change("en-US", vec![Main]),
        LocalizationChange::test_change("en-CA", vec![Main]),
    ], "#### At least 3 languages changed since 1.1.5:
For technical reasons, not all languages may be listed below. However, everything from \"Updated language translations\" and similar commits is listed, so the list is likely complete. You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en-CA`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-11b72e6873b6a42a2a1b92999e4225d7ff2921e323baa5e7b31fdc49471d9724)
- [English (`en-US`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-33a220e7f3b2e559ebec12cbf273da0c135bfade5a547e41e2bb5a66d75a01d2)"; "Android: 3 languages changed, likely complete")]
    #[test_case(Android, "v1.1.5", "v1.2.4", Incomplete, vec![
        LocalizationChange::test_change("en", vec![Main]),
        LocalizationChange::test_change("en-US", vec![Main]),
        LocalizationChange::test_change("en-CA", vec![Main]),
        LocalizationChange::test_change("en-AA", vec![Main]),
        LocalizationChange::test_change("en-BB", vec![Main]),

        LocalizationChange::test_change("en-CC", vec![Main]),
        LocalizationChange::test_change("en-DD", vec![Main]),
        LocalizationChange::test_change("en-EE", vec![Main]),
        LocalizationChange::test_change("en-FF", vec![Main]),
        LocalizationChange::test_change("en-GG", vec![Main]),

        LocalizationChange::test_change("en-HH", vec![Main]),
        LocalizationChange::test_change("en-II", vec![Main]),
        LocalizationChange::test_change("en-JJ", vec![Main]),
        LocalizationChange::test_change("en-KK", vec![Main]),
        LocalizationChange::test_change("en-LL", vec![Main]),

        LocalizationChange::test_change("en-MM", vec![Main]),
        LocalizationChange::test_change("en-NN", vec![Main]),
        LocalizationChange::test_change("en-OO", vec![Main]),
        LocalizationChange::test_change("en-PP", vec![Main]),
        LocalizationChange::test_change("en-QQ", vec![Main]),

        LocalizationChange::test_change("en-RR", vec![Main]),
    ], "#### At least 21 languages changed since 1.1.5:
:warning: For technical reasons, not all languages may be listed below. You can view the full comparison to 1.1.5 so far [here](https://github.com/signalapp/Signal-Android/compare/v1.1.5...v1.2.4).
[details=\"Show changes\"]
- [English (`en`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5e01f7d37a66e4ca03deefc205d8e7008661cdd0284a05aaba1858e6b7bf9103)
- [English (`en-AA`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-90d83c43cd0b4a891fa32e19f20c6c1c933d4627e2d70466c6de771fd8e7d5ea)
- [English (`en-BB`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-0538051d72d1e122563776d98638f915391c2b0e0627965fab1fd37d79bbe027)
- [English (`en-CA`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-11b72e6873b6a42a2a1b92999e4225d7ff2921e323baa5e7b31fdc49471d9724)
- [English (`en-CC`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-6d1f3e2b47a24280d0c1a2244667656c6ca5ac429d4e5ca4fa6e0dd9d21883f9)
- [English (`en-DD`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-835e31924911b451487467f65445296e538556689f0a349049deb46e54b4efdc)
- [English (`en-EE`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-5b62998ff46298e26bc02db71baa67c05eb294535820e9b131793eb8f40111ae)
- [English (`en-FF`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-41bde68259269b61d56f8011d6a6c0ced60a7509dd1f545e053f91ab254befa0)
- [English (`en-GG`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-a2c8f8883faf8422107e483d04d0f3135d7449581c8a03de7f2d5507404a264e)
- [English (`en-HH`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-19ee1c6b50a70f7ed36431d28d8537aa9ce6f11e0374b3c0976f65820ce55778)
- [English (`en-II`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-42c898889f9fc7175eecc63898739f5024b39e482b2919380c176437ac9b22fc)
- [English (`en-JJ`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-60a126ddd8b5b391468f28469cc0953c3d4d21020d4dc34b1f9ff6f67c6c680e)
- [English (`en-KK`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-7342487967d03331e2bad1197a40b1e7d1f34da6235c0736d32460e6351aaa50)
- [English (`en-LL`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-766a216d5eb0aa87ffed36c559ec6674f5c50ab234e2e91e71da6bfb86de939b)
- [English (`en-MM`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-b1ff359d6a6d77649a4261b2b05a515af6aedc44f7e575e950af537a4552c738)
- [English (`en-NN`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-fa942664e77daed786916de43eae073d6d01776eedd2c5010ec18d0ddcc5371d)
- [English (`en-OO`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-f2a68ac478f5458f61b8f99b92d7177f2bffab4a255b9299cb291d72ae6ef204)
- [English (`en-PP`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-2204ce5d73475a889fa5d44bfcb635868d9a4cad744767db8b91c79be1e989fd)
- [English (`en-QQ`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-e31e2b8f412ef0d7d8209f44c2efd3686cd9f9d51b71b9f83916dac0f7a3b144)
- [English (`en-RR`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-8e1942b7f79ae365c0629628ff5196b184f83b74ce16291f07a9efcc8941c597)
- [English (`en-US`)](https://github.com/signalapp/Signal-Android/compare/v1.1.5..v1.2.4#diff-33a220e7f3b2e559ebec12cbf273da0c135bfade5a547e41e2bb5a66d75a01d2)
[/details]"; "Android: 21 languages changed, incomplete")]
    fn to_string(
        platform: Platform,
        old_tag: &str,
        new_tag: &str,
        completeness: Completeness,
        changes: Vec<LocalizationChange>,
        result: &str,
    ) {
        let old_tag = Tag::new(old_tag);
        let new_tag = Tag::new(new_tag);

        let changes = LocalizationChanges {
            platform,
            old_tag: &old_tag,
            new_tag: &new_tag,
            completeness,
            unsorted_changes: LocalizationChange::unsorted_changes(changes),
        };

        assert_str_eq!(changes.to_string(), result);
    }
}
