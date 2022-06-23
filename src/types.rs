use serde_derive::Deserialize;

pub mod github {
    use semver::Version;

    use crate::utils;

    use super::*;

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
}

pub mod discourse {
    use serde_json::Value;
    use std::collections::HashMap;

    use super::*;

    #[derive(Deserialize, Debug)]
    pub struct PostApiResponse {
        pub post_number: Option<u64>,

        #[serde(flatten)]
        pub other_fields: HashMap<String, Value>,
    }

    #[derive(Deserialize, Debug)]
    pub struct TopicResponse {
        pub post_stream: PostStream,
    }

    #[derive(Deserialize, Debug)]
    pub struct PostStream {
        pub posts: Vec<Post>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Post {
        pub topic_id: u64,
    }
}
