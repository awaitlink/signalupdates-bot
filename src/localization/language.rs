use std::fmt;

use anyhow::{anyhow, bail};
use locale_codes::language;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(try_from = "&str", into = "String")]
pub struct Language {
    pub language_reference_name: String,
    pub language_code: String,
    pub region_code: Option<String>,
}

impl Default for Language {
    fn default() -> Self {
        Self {
            language_reference_name: String::from("English"),
            language_code: String::from("en"),
            region_code: None,
        }
    }
}

impl Language {
    pub fn full_code(&self) -> String {
        match &self.region_code {
            Some(region_code) => format!("{}-{region_code}", self.language_code),
            None => self.language_code.to_string(),
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (`{}`)",
            self.language_reference_name,
            self.full_code()
        )
    }
}

impl From<Language> for String {
    fn from(language: Language) -> Self {
        language.full_code()
    }
}

impl TryFrom<&str> for Language {
    type Error = anyhow::Error;

    fn try_from(code: &str) -> Result<Self, Self::Error> {
        let canonical_code = code.replace("-r", "_").replace('-', "_");
        let parts = canonical_code.split('_').collect::<Vec<_>>();

        match parts.len() {
            1..=2 => {
                let language_code = parts[0].to_string();
                if !(2..=3).contains(&language_code.len()) {
                    bail!("language code length is not in range 2..=3");
                }

                let language_reference_name = language::lookup(&language_code)
                    .ok_or_else(|| anyhow!("could not look up language by code"))?
                    .reference_name
                    .clone();

                Ok(Self {
                    language_reference_name,
                    language_code,
                    region_code: parts.get(1).map(|code| code.to_string()),
                })
            }
            _ => bail!("incorrect count of code parts"),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::{assert_eq, assert_str_eq};
    use test_case::test_case;

    use super::*;

    #[test_case("en", "English (`en`)"; "en")]
    #[test_case("en_US", "English (`en-US`)"; "en underscore US")]
    #[test_case("en-US", "English (`en-US`)"; "en dash US")]
    #[test_case("en-rUS", "English (`en-US`)"; "en dash r US")]
    #[test_case("eo", "Esperanto (`eo`)"; "eo")]
    #[test_case("yue", "Yue Chinese (`yue`)"; "yue")]
    #[test_case("kab", "Kabyle (`kab`)"; "kab")]
    #[test_case("pt_BR", "Portuguese (`pt-BR`)"; "pt underscore BR")]
    #[test_case("pt_PT", "Portuguese (`pt-PT`)"; "pt underscore PT")]
    #[test_case("zh_CN", "Chinese (`zh-CN`)"; "zh underscore CN")]
    #[test_case("zh_TW", "Chinese (`zh-TW`)"; "zh underscore TW")]
    #[test_case("pa-rPK", "Panjabi (`pa-PK`)"; "pa dash r PK")]
    #[test_case("qu-rEC", "Quechua (`qu-EC`)"; "qu dash r EC")]
    fn from_code_some(code: &str, result: &str) {
        assert_str_eq!(Language::try_from(code).unwrap().to_string(), result);
    }

    // Some of the values-* folders in Signal Android are not for localization.
    #[test_case("land")]
    #[test_case("ldrtl")]
    #[test_case("night")]
    #[test_case("v9")]
    fn from_code_none(code: &str) {
        assert!(Language::try_from(code).is_err());
    }

    #[test_case(&["pt_PT", "pt_BR", "en_US", "eo", "en"], &["en", "en_US", "eo", "pt_BR", "pt_PT"]; "basic test")]
    fn ord(input: &[&str], output: &[&str]) {
        let map = |x: &[&str]| {
            x.iter()
                .map(|code| Language::try_from(*code).unwrap())
                .collect::<Vec<_>>()
        };

        let mut input = map(input);
        let output = map(output);

        input.sort_unstable();
        assert_eq!(input, output);
    }
}
