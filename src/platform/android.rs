use anyhow::Context;
use regex::RegexBuilder;

#[derive(Debug, PartialEq)]
pub struct BuildConfiguration {
    pub canonical_version_code: u64,
    pub current_hotfix_version: u64,
    pub max_hotfix_versions: u64,
}

impl BuildConfiguration {
    pub fn from_app_build_gradle_kts(text: &str) -> anyhow::Result<Self> {
        // Canonical version code

        let canonical_version_code =
            Self::get_u64(text, r"val \s+ canonicalVersionCode \s* = \s* (\d+)", 1)
                .context("couldn't get canonical_version_code")?;

        // Current hotfix version

        let current_hotfix_version =
            Self::get_u64(text, r"val \s+ currentHotfixVersion \s* = \s* (\d+)", 1)
                .context("couldn't get current_hotfix_version")?;

        // Max hotfix versions

        let max_hotfix_versions =
            Self::get_u64(text, r"val \s+ maxHotfixVersions \s* = \s* (\d+)", 1)
                .context("couldn't get max_hotfix_versions")?;

        Ok(BuildConfiguration {
            canonical_version_code,
            current_hotfix_version,
            max_hotfix_versions,
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

                val canonicalVersionCode = 1428
                val canonicalVersionName = "7.9.6"
                val currentHotfixVersion = 0
                val maxHotfixVersions = 100

                more things here"#
            )
            .unwrap(),
            BuildConfiguration {
                canonical_version_code: 1428,
                current_hotfix_version: 0,
                max_hotfix_versions: 100,
            }
        );
    }
}
