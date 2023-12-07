use std::collections::HashMap;

use anyhow::Context;
use regex::RegexBuilder;

#[derive(Debug, PartialEq)]
pub struct BuildConfiguration {
    pub canonical_version_code: u64,
    pub postfix_size: u64,
    pub abi_postfixes: HashMap<String, u64>,
}

impl BuildConfiguration {
    pub fn from_app_build_gradle_kts(text: &str) -> anyhow::Result<Self> {
        // Canonical version code

        let canonical_version_code =
            Self::get_u64(text, r"val \s+ canonicalVersionCode \s* = \s* (\d+)", 1)
                .context("couldn't get canonical_version_code")?;

        // Postfix size

        let postfix_size = Self::get_u64(text, r"val \s+ postFixSize \s* = \s* (\d+)", 1)
            .context("couldn't get postfix_size")?;

        // ABI postfixes

        let map_regex_string = r#"\s* "([^"]+)" \s+ to \s+ (\d+) \s* ,? \s* \n?"#;

        let abi_postfixes_regex = RegexBuilder::new(
            &(String::from(
                r#"val \s+ abiPostFix \s* : \s* Map<String, \s* Int> \s* = \s* mapOf \s* \( \n? (("#,
            ) + map_regex_string
                + r#")+) \)"#),
        )
        .ignore_whitespace(true)
        .build()
        .context("couldn't compile abi_postfixes_regex")?;

        let caps = abi_postfixes_regex
            .captures(text)
            .context("couldn't find abi postfixes in file")?;

        let map_regex = RegexBuilder::new(map_regex_string)
            .ignore_whitespace(true)
            .build()
            .context("couldn't compile map_regex")?;

        let mut abi_postfixes = HashMap::new();
        let map_caps = map_regex.captures_iter(&caps[1]).map(|c| c.extract());
        for (_, [k, v]) in map_caps {
            abi_postfixes.insert(
                k.to_owned(),
                str::parse(v).context("couldn't parse abi postfix as u64")?,
            );
        }

        Ok(BuildConfiguration {
            canonical_version_code,
            postfix_size,
            abi_postfixes,
        })
    }

    fn get_u64(text: &str, regex: &str, capture_number: usize) -> anyhow::Result<u64> {
        let re = RegexBuilder::new(regex)
            .ignore_whitespace(true)
            .build()
            .context("couldn't compile regex")?;

        let caps = re.captures(text).context("couldn't find match in text")?;
        str::parse(&caps[capture_number]).context("couldn't parse as u64")
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn build_configuration() {
        assert_eq!(
            BuildConfiguration::from_app_build_gradle_kts(
                r#"
                some other; things...

                val canonicalVersionCode = 1366
                val canonicalVersionName = "6.42.0"

                val postFixSize = 100
                val abiPostFix: Map<String, Int> = mapOf(
                "universal" to 0,
                "armeabi-v7a" to 1,
                "arm64-v8a" to 2,
                "x86" to 3,
                "x86_64" to 4
                )

                more things here"#
            )
            .unwrap(),
            BuildConfiguration {
                canonical_version_code: 1366,
                abi_postfixes: {
                    let mut map = HashMap::new();
                    map.insert(String::from("universal"), 0);
                    map.insert(String::from("armeabi-v7a"), 1);
                    map.insert(String::from("arm64-v8a"), 2);
                    map.insert(String::from("x86"), 3);
                    map.insert(String::from("x86_64"), 4);
                    map
                },
                postfix_size: 100
            }
        );
    }
}
