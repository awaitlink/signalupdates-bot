use anyhow::{anyhow, Context};
use semver::Version;
use serde::Serialize;
use serde_derive::Deserialize;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
}

impl Tag {
    pub fn exact_version_string(&self) -> String {
        self.name.replace('v', "")
    }

    pub fn to_version(&self) -> anyhow::Result<Version> {
        lenient_semver::parse(&self.name)
            .map_err(|e| anyhow!(e.to_string()))
            .context("could not parse version from tag")
    }

    pub fn from_exact_version_string(version: &str) -> Self {
        Self {
            name: format!("v{version}"),
        }
    }
}

#[cfg(test)]
impl Tag {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub total_commits: usize,
    pub commits: Vec<Commit>,
    pub files: Option<Vec<File>>,
}

impl Comparison {
    /// GitHub API only returns at most this many files in a comparison, despite
    /// https://docs.github.com/en/rest/commits/commits#compare-two-commits
    /// saying that it always returns all.
    pub const GITHUB_API_MAX_FILES: usize = 300;

    /// Indicates whether `files` in this comparison is likely complete or not.
    /// (see also [`GITHUB_API_MAX_FILES`]).
    ///
    /// Returns `None` if `self.files` is `None`.
    pub fn are_files_likely_complete(&self) -> Option<bool> {
        self.files
            .as_ref()
            .map(|files| files.len() != Self::GITHUB_API_MAX_FILES)
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub sha: String,
    pub commit: CommitData,
    pub files: Option<Vec<File>>,
}

impl Commit {
    /// GitHub API only returns at most this many files in a commit, according to
    /// https://docs.github.com/en/rest/commits/commits#get-a-commit.
    pub const GITHUB_API_MAX_FILES: usize = 3000;

    /// Indicates whether `files` in this commit is likely complete or not.
    /// (see also [`GITHUB_API_MAX_FILES`]).
    ///
    /// Returns `None` if `self.files` is `None`.
    pub fn are_files_likely_complete(&self) -> Option<bool> {
        self.files
            .as_ref()
            .map(|files| files.len() != Self::GITHUB_API_MAX_FILES)
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CommitData {
    pub message: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub filename: String,
}

#[cfg(test)]
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ContentsEntry {
    pub path: String,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use test_case::test_case;

    use super::*;

    fn test_version(pre: Option<&str>, build: Option<&str>) -> Version {
        use semver::{BuildMetadata, Prerelease};

        Version {
            major: 1,
            minor: 2,
            patch: 3,
            pre: match pre {
                Some(pre) => Prerelease::new(pre).unwrap(),
                None => Prerelease::EMPTY,
            },
            build: match build {
                Some(build) => BuildMetadata::new(build).unwrap(),
                None => BuildMetadata::EMPTY,
            },
        }
    }

    #[test_case("v1.2.3", test_version(None, None); "3 digits with v")]
    #[test_case("1.2.3", test_version(None, None); "3 digits without v")]
    #[test_case("v1.2.3.4", test_version(None, Some("4")); "4 digits with v")]
    #[test_case("1.2.3.4", test_version(None, Some("4")); "4 digits without v")]
    #[test_case("v1.2.3-beta.1", test_version(Some("beta.1"), None); "3 digits beta with v")]
    #[test_case("1.2.3.4-beta", test_version(Some("beta"), Some("4")); "4 digits beta without v")]
    fn version_from_tag(tag: &str, result: Version) {
        let version: Version = Tag::new(tag).to_version().unwrap();

        assert_eq!(version, result);
    }

    #[test_case(
      &["v1.2.3", "v1.2.5", "v1.1.3", "v1.2.4"],
      &["v1.1.3", "v1.2.3", "v1.2.4", "v1.2.5"];
      "three digits"
    )]
    #[test_case(
      &["1.2.3.3-beta", "1.1.3.3-beta", "1.2.3.5-beta", "1.2.3.4-beta"],
      &["1.1.3.3-beta", "1.2.3.3-beta", "1.2.3.4-beta", "1.2.3.5-beta"];
      "four digits beta"
    )]
    #[test_case(
      &["v1.2.3-beta.1", "v1.2.3-beta.3", "v1.1.3-beta.1", "v1.2.3-beta.2"],
      &["v1.1.3-beta.1", "v1.2.3-beta.1", "v1.2.3-beta.2", "v1.2.3-beta.3"];
      "three digits beta dot digit"
    )]
    fn compare_versions(input: &[&str], output: &[&str]) {
        let map = |x: &[&str]| {
            x.iter()
                .map(|tag_name| Tag::new(tag_name.to_string()).to_version().unwrap())
                .collect::<Vec<_>>()
        };

        let mut input: Vec<Version> = map(input);
        let output = map(output);

        input.sort_unstable();
        assert_eq!(input, output);
    }

    #[test]
    fn comparison_deserialization() {
        // Example from https://docs.github.com/en/rest/commits/commits#compare-two-commits
        let input = include_str!("comparison_example.json");

        assert_eq!(
            serde_json::from_str::<Comparison>(input).unwrap(),
            Comparison {
                total_commits: 1,
                commits: vec![Commit {
                    sha: "6dcb09b5b57875f334f61aebed695e2e4193db5e".to_string(),
                    commit: CommitData {
                        message: "Fix all the bugs".to_string()
                    },
                    files: None,
                }],
                files: Some(vec![File {
                    filename: "file1.txt".to_string()
                }])
            }
        );
    }
}
