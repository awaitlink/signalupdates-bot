use locale_codes::{country, language, region};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Language {
    pub language_reference_name: String,
    pub region: Option<Region>,
    pub language_code: String,
}

impl Default for Language {
    fn default() -> Self {
        Self {
            language_reference_name: String::from("English"),
            region: None,
            language_code: String::from("en"),
        }
    }
}

impl Language {
    pub fn from_code(code: &str) -> Option<Self> {
        let canonical_code = code.replace("-r", "_").replace('-', "_");
        let parts = canonical_code.split('_').collect::<Vec<_>>();

        match parts.len() {
            1..=2 => {
                let language_code = parts[0].to_string();
                if !(2..=3).contains(&language_code.len()) {
                    return None;
                }

                let language_reference_name =
                    language::lookup(&language_code)?.reference_name.clone();

                let region = match parts.get(1) {
                    Some(territory_code) => Region::from_territory_code(territory_code),
                    None => None,
                };

                Some(Self {
                    language_reference_name,
                    region,
                    language_code,
                })
            }
            _ => None,
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.region {
            Some(region) => write!(
                f,
                "{} ({}) (`{}-{}`)",
                self.language_reference_name, region.name, self.language_code, region.code
            ),
            None => write!(
                f,
                "{} (`{}`)",
                self.language_reference_name, self.language_code
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Region {
    pub name: String,
    pub code: String,
}

impl Region {
    pub fn from_territory_code(territory_code: &str) -> Option<Self> {
        let country_code = country::lookup(territory_code)?.country_code;
        let name = region::lookup(country_code)?.name.clone();

        Some(Region {
            name,
            code: territory_code.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("en", "English (`en`)"; "en")]
    #[test_case("en_US", "English (United States of America) (`en-US`)"; "en underscore US")]
    #[test_case("en-US", "English (United States of America) (`en-US`)"; "en dash US")]
    #[test_case("en-rUS", "English (United States of America) (`en-US`)"; "en dash r US")]
    #[test_case("eo", "Esperanto (`eo`)"; "eo")]
    #[test_case("yue", "Yue Chinese (`yue`)"; "yue")]
    #[test_case("kab", "Kabyle (`kab`)"; "kab")]
    #[test_case("pt_BR", "Portuguese (Brazil) (`pt-BR`)"; "pt underscore BR")]
    #[test_case("pt_PT", "Portuguese (Portugal) (`pt-PT`)"; "pt underscore PT")]
    #[test_case("zh_CN", "Chinese (China) (`zh-CN`)"; "zh underscore CN")]
    #[test_case("zh_TW", "Chinese (Taiwan, Province of China) (`zh-TW`)"; "zh underscore TW")]
    #[test_case("pa-rPK", "Panjabi (Pakistan) (`pa-PK`)"; "pa dash r PK")]
    #[test_case("qu-rEC", "Quechua (Ecuador) (`qu-EC`)"; "qu dash r EC")]
    fn language_from_code(code: &str, result: &str) {
        assert_eq!(Language::from_code(code).unwrap().to_string(), result);
    }

    // Some of the values-* folders in Signal Android are not for localization.
    #[test_case("land")]
    #[test_case("ldrtl")]
    #[test_case("night")]
    #[test_case("v9")]
    fn language_from_code_none(code: &str) {
        assert!(Language::from_code(code).is_none());
    }

    #[test_case(&["pt_PT", "pt_BR", "en_US", "eo", "en"], &["en", "en_US", "eo", "pt_BR", "pt_PT"]; "basic test")]
    fn language_ord(input: &[&str], output: &[&str]) {
        let map = |x: &[&str]| {
            x.iter()
                .map(|code| Language::from_code(code).unwrap())
                .collect::<Vec<_>>()
        };

        let mut input = map(input);
        let output = map(output);

        input.sort_unstable();
        assert_eq!(input, output);
    }
}
