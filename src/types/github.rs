use semver::Version;

use super::*;
use crate::utils;

#[derive(Deserialize, Debug, Clone)]
pub struct Tag {
    pub name: String,
}

impl TryFrom<Tag> for Version {
    type Error = anyhow::Error;

    fn try_from(tag: Tag) -> Result<Self, Self::Error> {
        utils::version_from_tag(&tag.name)
    }
}

impl TryFrom<&Tag> for Version {
    type Error = anyhow::Error;

    fn try_from(tag: &Tag) -> Result<Self, Self::Error> {
        utils::version_from_tag(&tag.name)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Comparison {
    pub commits: Vec<Commit>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Commit {
    pub sha: String,
    pub commit: CommitData,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CommitData {
    pub message: String,
}
