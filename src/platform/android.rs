use std::collections::HashMap;

use anyhow::Context;
use regex::Regex;

#[derive(Debug)]
pub struct BuildConfiguration {
    pub canonical_version_code: u64,
    pub postfix_size: u64,
    pub abi_postfixes: HashMap<String, u64>,
}

impl BuildConfiguration {
    pub fn from_app_build_gradle(text: &str) -> anyhow::Result<Self> {
        // Canonical version code

        let canonical_version_code =
            Self::get_u64(text, r"def\s+canonicalVersionCode\s*=\s*(\d+)", 1)
                .context("couldn't get canonical_version_code")?;

        // Postfix size

        let postfix_size = Self::get_u64(text, r"def\s+postFixSize\s*=\s*(\d+)", 1)
            .context("couldn't get postfix_size")?;

        // ABI postfixes

        let abi_postfixes_regex = Regex::new(r"def\s+abiPostFix\s*=\s*\[([^\]]+\n?)+\]")
            .context("couldn't compile abi_postfixes_regex")?;

        let caps = abi_postfixes_regex
            .captures(text)
            .context("couldn't find abi postfixes in file")?;

        let json = (String::from("{") + &caps[1] + "}").replace('\'', "\"");

        let abi_postfixes: HashMap<String, u64> =
            serde_json::from_str(&json).context("couldn't parse abi postfixes")?;

        Ok(BuildConfiguration {
            canonical_version_code,
            postfix_size,
            abi_postfixes,
        })
    }

    fn get_u64(text: &str, regex: &str, capture_number: usize) -> anyhow::Result<u64> {
        let re = Regex::new(regex).context("couldn't compile regex")?;
        let caps = re.captures(text).context("couldn't find match in text")?;
        str::parse(&caps[capture_number]).context("couldn't parse as u64")
    }
}
